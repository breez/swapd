use std::time::Duration;
use std::{future::Future, pin::pin, sync::Arc, time::SystemTime};

use futures::future::{FusedFuture, FutureExt};
use futures::{stream::FuturesUnordered, StreamExt};
use thiserror::Error;
use tokio::join;
use tracing::{debug, error, field, instrument, trace};

use crate::chain::BroadcastError;
use crate::{
    chain::{
        ChainClient, ChainError, ChainRepository, ChainRepositoryError, FeeEstimateError,
        FeeEstimator,
    },
    swap::{CreateRedeemTxError, GetSwapsError, PrivateKeyProvider, SwapRepository, SwapService},
    wallet::{Wallet, WalletError},
};

use super::service::{RedeemService, RedeemServiceError};
use super::Redeemable;
use super::{repository::RedeemRepository, Redeem, RedeemRepositoryError};

const MIN_REPLACEMENT_DIFF_SAT_PER_KW: u32 = 250;

#[derive(Debug, Error)]
pub enum RedeemError {
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

pub struct RedeemMonitorParams<CC, CR, FE, SR, P, RR, W>
where
    CC: ChainClient,
    CR: ChainRepository,
    FE: FeeEstimator,
    SR: SwapRepository,
    P: PrivateKeyProvider,
    RR: RedeemRepository,
    W: Wallet,
{
    pub chain_client: Arc<CC>,
    pub fee_estimator: Arc<FE>,
    pub poll_interval: Duration,
    pub swap_service: Arc<SwapService<P>>,
    pub redeem_repository: Arc<RR>,
    pub redeem_service: Arc<RedeemService<CR, SR>>,
    pub wallet: Arc<W>,
}

pub struct RedeemMonitor<CC, CR, FE, SR, P, RR, W>
where
    CC: ChainClient,
    CR: ChainRepository,
    FE: FeeEstimator,
    SR: SwapRepository,
    P: PrivateKeyProvider,
    RR: RedeemRepository,
    W: Wallet,
{
    chain_client: Arc<CC>,
    fee_estimator: Arc<FE>,
    poll_interval: Duration,
    swap_service: Arc<SwapService<P>>,
    redeem_repository: Arc<RR>,
    redeem_service: Arc<RedeemService<CR, SR>>,
    wallet: Arc<W>,
}

impl<CC, CR, FE, SR, P, RR, W> RedeemMonitor<CC, CR, FE, SR, P, RR, W>
where
    CC: ChainClient,
    CR: ChainRepository,
    FE: FeeEstimator,
    SR: SwapRepository,
    P: PrivateKeyProvider,
    RR: RedeemRepository,
    W: Wallet,
{
    pub fn new(params: RedeemMonitorParams<CC, CR, FE, SR, P, RR, W>) -> Self {
        Self {
            chain_client: params.chain_client,
            fee_estimator: params.fee_estimator,
            poll_interval: params.poll_interval,
            swap_service: params.swap_service,
            redeem_repository: params.redeem_repository,
            redeem_service: params.redeem_service,
            wallet: params.wallet,
        }
    }

    pub async fn start<F: Future<Output = ()>>(&self, signal: F) -> Result<(), RedeemError> {
        let mut sig = pin!(signal.fuse());
        if sig.is_terminated() {
            return Ok(());
        }

        loop {
            debug!("starting chain sync task");
            match self.do_sync().await {
                Ok(_) => debug!("chain sync task completed succesfully"),
                Err(e) => error!("chain sync task failed with: {:?}", e),
            }

            tokio::select! {
                _ = &mut sig => {
                    debug!("redeem monitor shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {}
            }
        }

        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    async fn do_sync(&self) -> Result<(), RedeemError> {
        let redeemable_swaps = self.redeem_service.list_redeemable().await?;
        let current_height = self.chain_client.get_blockheight().await?;

        let mut tasks = FuturesUnordered::new();
        for redeemable_swap in redeemable_swaps {
            tasks.push(self.redeem_swap(redeemable_swap, current_height))
        }

        while let Some(result) = tasks.next().await {
            if let Err(e) = result {
                // TODO: Log which task it was somehow.
                error!("redeem task errored with: {:?}", e);
            }
        }

        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    async fn redeem_swap(
        &self,
        redeemable: Redeemable,
        current_height: u64,
    ) -> Result<(), RedeemError> {
        // Only redeem utxos that were paid as part of an invoice payment. Users
        // sometimes send funds to the same address multiple times, even though
        // it is no longer safe to do so, because it's not obvious to a user
        // a P2WSH address should not be reused. Be a good citizen and allow the
        // user to refund those utxos. Note these utxos can still be redeemed
        // manually by the swap server if needed.
        let utxos: Vec<_> = redeemable
            .utxos
            .iter()
            .filter(|utxo| utxo.paid_with_request.is_some())
            .map(|utxo| &utxo.utxo)
            .cloned()
            .collect();
        if utxos.is_empty() {
            return Ok(());
        }

        // NOTE: This unwrap only works because the utxos vec is not empty!
        let min_conf_height = utxos.iter().map(|u| u.block_height).min().unwrap();

        // Blocks left gives a sense of urgency for this redeem.
        let blocks_left = (redeemable.swap.public.lock_time as i32)
            - (current_height.saturating_sub(min_conf_height) as i32);
        let fee_estimate_fut = self.fee_estimator.estimate_fee(blocks_left);
        let last_redeem_fut = self
            .redeem_repository
            .get_last_redeem(&redeemable.swap.public.hash);
        let (fee_estimate_res, last_redeem_res) = join!(fee_estimate_fut, last_redeem_fut);
        let fee_estimate = fee_estimate_res?;
        let destination_address = if let Some(last_redeem) = last_redeem_res? {
            // if the previous fee rate is still sufficient and it spends the
            // same utxos, attempt to rebroadcast the tx and return. Utxos can
            // be different, for example if there has been a reorg in the meantime.
            // Note that until cluster mempool is available, creating transactions
            // with different input sets could lead to pinning ourselves. This
            // should not happen most of the time, but it is a possibility.
            let sorted_new: Vec<_> = utxos.iter().map(|utxo| utxo.outpoint).collect();
            let sorted_old: Vec<_> = last_redeem
                .tx
                .input
                .iter()
                .map(|input| input.previous_output)
                .collect();
            if last_redeem.fee_per_kw + MIN_REPLACEMENT_DIFF_SAT_PER_KW > fee_estimate.sat_per_kw
                && sorted_new == sorted_old
            {
                debug!(
                    fee_per_kw = last_redeem.fee_per_kw,
                    hash = field::display(redeemable.swap.public.hash),
                    tx_id = field::display(last_redeem.tx.txid()),
                    "rebroadcasting redeem tx"
                );
                if let Err(e) = self.chain_client.broadcast_tx(last_redeem.tx).await {
                    match e {
                        crate::chain::BroadcastError::InsufficientFeeRejectingReplacement(e) => {
                            trace!(
                                "got expected error for rebroadcast: 'insufficient fee, rejecting replacement {}'",
                                e
                            )
                        }
                        _ => return Err(e.into()),
                    }
                }
                return Ok(());
            }

            // If not, replace with the same destination address.
            last_redeem.destination_address
        } else {
            self.wallet.new_address().await?
        };

        let redeem_tx = self.swap_service.create_redeem_tx(
            &redeemable.swap,
            &utxos,
            &fee_estimate,
            current_height,
            &redeemable.preimage,
            destination_address.clone(),
        )?;
        debug!(
            fee_per_kw = fee_estimate.sat_per_kw,
            hash = field::display(redeemable.swap.public.hash),
            tx_id = field::display(redeem_tx.txid()),
            "broadcasting new redeem tx"
        );
        self.chain_client.broadcast_tx(redeem_tx.clone()).await?;
        self.redeem_repository
            .add_redeem(&Redeem {
                creation_time: SystemTime::now(),
                destination_address,
                fee_per_kw: fee_estimate.sat_per_kw,
                tx: redeem_tx,
                swap_hash: redeemable.swap.public.hash,
            })
            .await?;
        Ok(())
    }
}

impl From<bitcoin::address::Error> for RedeemError {
    fn from(value: bitcoin::address::Error) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<ChainRepositoryError> for RedeemError {
    fn from(value: ChainRepositoryError) -> Self {
        match value {
            ChainRepositoryError::MultipleTips => RedeemError::General(Box::new(value)),
            ChainRepositoryError::General(e) => RedeemError::General(e),
        }
    }
}

impl From<ChainError> for RedeemError {
    fn from(value: ChainError) -> Self {
        match value {
            ChainError::General(e) => RedeemError::General(e),
            ChainError::Database(_) => RedeemError::General(Box::new(value)),
            ChainError::EmptyChain => RedeemError::General(Box::new(value)),
            ChainError::InvalidChain => RedeemError::General(Box::new(value)),
            ChainError::BlockNotFound => RedeemError::General(Box::new(value)),
        }
    }
}

impl From<BroadcastError> for RedeemError {
    fn from(value: BroadcastError) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<CreateRedeemTxError> for RedeemError {
    fn from(value: CreateRedeemTxError) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<FeeEstimateError> for RedeemError {
    fn from(value: FeeEstimateError) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<GetSwapsError> for RedeemError {
    fn from(value: GetSwapsError) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<RedeemRepositoryError> for RedeemError {
    fn from(value: RedeemRepositoryError) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<WalletError> for RedeemError {
    fn from(value: WalletError) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<RedeemServiceError> for RedeemError {
    fn from(value: RedeemServiceError) -> Self {
        RedeemError::General(Box::new(value))
    }
}
