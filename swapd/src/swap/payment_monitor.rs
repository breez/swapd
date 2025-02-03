use std::{sync::Arc, time::Duration};

use tokio_util::sync::CancellationToken;
use tracing::{debug, debug_span, error, field};

use crate::{
    lightning::{LightningClient, LightningError, PaymentResult, PaymentState},
    swap::{PaymentAttempt, SwapRepository},
};

pub struct HistoricalPaymentMonitor<S, L> {
    lightning_client: Arc<L>,
    payment_attempts: Vec<PaymentAttempt>,
    poll_interval: Duration,
    swap_repository: Arc<S>,
}

impl<S, L> HistoricalPaymentMonitor<S, L>
where
    S: SwapRepository,
    L: LightningClient,
{
    pub fn new(lightning_client: Arc<L>, poll_interval: Duration, swap_repository: Arc<S>) -> Self {
        HistoricalPaymentMonitor {
            lightning_client,
            payment_attempts: Vec::new(),
            poll_interval,
            swap_repository,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let unhandled_attempts = self
            .swap_repository
            .get_unhandled_payment_attempts()
            .await?;
        let pending_attempts = self.do_check_payments(&unhandled_attempts).await?;
        self.payment_attempts = pending_attempts;
        Ok(())
    }

    pub async fn start(
        &mut self,
        token: CancellationToken,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            if token.is_cancelled() {
                return Ok(());
            }

            if self.payment_attempts.is_empty() {
                debug!("finished checking historical payments");
                return Ok(());
            }

            match self.do_check_payments(&self.payment_attempts).await {
                Ok(pending_attempts) => self.payment_attempts = pending_attempts,
                Err(e) => error!("failed to check historical payments: {:?}", e),
            }

            tokio::select! {
                _ = token.cancelled() => {
                    debug!("historical payments monitor shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {}
            }
        }

        Ok(())
    }

    pub async fn do_check_payments(
        &self,
        payment_attempts: &[PaymentAttempt],
    ) -> Result<Vec<PaymentAttempt>, Box<dyn std::error::Error>> {
        let mut pending_attempts = Vec::new();
        for attempt in payment_attempts {
            debug_span!(
                "checking_payment",
                payment_hash = field::display(&attempt.payment_hash)
            );
            let swap_state = self
                .swap_repository
                .get_swap_by_hash(&attempt.payment_hash)
                .await?;
            let state = match self
                .lightning_client
                .get_payment_state(attempt.payment_hash, &attempt.label)
                .await
            {
                Ok(state) => state,
                Err(LightningError::PaymentNotFound) => {
                    debug!("historical swap payment not found, removing from pending payments",);
                    self.swap_repository
                        .unlock_add_payment_result(
                            &swap_state.swap,
                            &attempt.label,
                            &PaymentResult::Failure {
                                error: "cancelled".to_string(),
                            },
                        )
                        .await?;

                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            match state {
                PaymentState::Success { preimage } => {
                    debug!("historical swap payment was successful",);
                    self.swap_repository
                        .unlock_add_payment_result(
                            &swap_state.swap,
                            &attempt.label,
                            &PaymentResult::Success { preimage },
                        )
                        .await?;
                }
                PaymentState::Failure { error } => {
                    debug!("historical swap payment failed with error: {}", error,);
                    self.swap_repository
                        .unlock_add_payment_result(
                            &swap_state.swap,
                            &attempt.label,
                            &PaymentResult::Failure { error },
                        )
                        .await?;
                }
                PaymentState::Pending => pending_attempts.push(attempt.clone()),
            }
        }

        Ok(pending_attempts)
    }
}
