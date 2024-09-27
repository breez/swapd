use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::time::Duration;
use std::{future::Future, pin::pin, sync::Arc};

use bitcoin::OutPoint;
use futures::future::{FusedFuture, FutureExt};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::join;
use tracing::{debug, error, field, instrument};

use crate::chain::BroadcastError;
use crate::swap::RedeemableUtxo;
use crate::{
    chain::{
        ChainClient, ChainError, ChainRepository, ChainRepositoryError, FeeEstimateError,
        FeeEstimator,
    },
    swap::{CreateRedeemTxError, GetSwapsError, PrivateKeyProvider, SwapRepository},
    wallet::{Wallet, WalletError},
};

use super::service::{RedeemService, RedeemServiceError};
use super::RedeemError;
use super::{repository::RedeemRepository, Redeem, RedeemRepositoryError};

const MIN_REPLACEMENT_DIFF_SAT_PER_KW: u32 = 250;
type RedeemFut<'a> = Pin<
    Box<
        dyn Future<Output = (Result<(), RedeemError>, Option<Redeem>, Vec<RedeemableUtxo>)>
            + Send
            + 'a,
    >,
>;

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
    pub redeem_repository: Arc<RR>,
    pub redeem_service: Arc<RedeemService<CC, CR, RR, SR, P>>,
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
    redeem_repository: Arc<RR>,
    redeem_service: Arc<RedeemService<CC, CR, RR, SR, P>>,
    wallet: Arc<W>,
}

impl<CC, CR, FE, SR, P, RR, W> RedeemMonitor<CC, CR, FE, SR, P, RR, W>
where
    CC: ChainClient + Send + Sync,
    CR: ChainRepository + Send + Sync,
    FE: FeeEstimator + Send + Sync,
    SR: SwapRepository + Send + Sync,
    P: PrivateKeyProvider + Send + Sync,
    RR: RedeemRepository + Send + Sync,
    W: Wallet + Send + Sync,
{
    pub fn new(params: RedeemMonitorParams<CC, CR, FE, SR, P, RR, W>) -> Self {
        Self {
            chain_client: params.chain_client,
            fee_estimator: params.fee_estimator,
            poll_interval: params.poll_interval,
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
            debug!("starting redeem task");
            match self.do_redeem().await {
                Ok(_) => debug!("redeem task completed succesfully"),
                Err(e) => error!("redeem task failed with: {:?}", e),
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
    async fn do_redeem(&self) -> Result<(), RedeemError> {
        let current_height = self.chain_client.get_blockheight().await?;
        let redeemables = self.redeem_service.list_redeemable().await?;
        let redeemables: HashMap<_, _> = redeemables
            .into_iter()
            .map(|redeemable| (redeemable.utxo.outpoint, redeemable))
            .collect();
        let outpoints: Vec<_> = redeemables.keys().cloned().collect();

        // Get existing redeems, sorted by highest fee rate and then creation time.
        let redeems = self.redeem_repository.get_redeems(&outpoints).await?;

        // First remove all the outpoints where there is already an in-progress
        // redeem transaction published. These in-progress redeem transactions
        // will be rechecked for fees below.
        let mut recheck_redeems = Vec::new();
        let mut unhandled_outpoints: HashSet<OutPoint> = outpoints.iter().cloned().collect();
        for redeem in redeems {
            let outpoints: Vec<_> = redeem
                .tx
                .input
                .iter()
                .map(|input| input.previous_output)
                .collect();
            // Only process this redeem if it is still spending valid outputs.
            if !outpoints
                .iter()
                .all(|outpoint| redeemables.contains_key(outpoint))
            {
                continue;
            }

            let mut current_redeemables: Vec<RedeemableUtxo> = Vec::new();
            for outpoint in &outpoints {
                unhandled_outpoints.remove(outpoint);
                current_redeemables.push(
                    redeemables
                        .get(outpoint)
                        .expect("missing expected redeemable utxo in map")
                        .clone(),
                );
            }

            recheck_redeems.push((redeem, current_redeemables));
        }

        // Now group the remaining utxos by swap. Note grouping by swap is
        // pretty much an arbitrary decision. they might as well be grouped by
        // remaining timelock to save on fees.
        let mut swaps = HashMap::new();
        for unhandled_outpoint in &unhandled_outpoints {
            let redeemable = redeemables
                .get(unhandled_outpoint)
                .expect("missing expected redeemable utxo in map");

            // Be a good citizen and don't redeem any funds that were not paid
            // over lightning. That can happen if the user sends another onchain
            // transaction to the same address multiple times. Users may not
            // know this is unsafe for them to do.
            if redeemable.paid_with_request.is_none() {
                // TODO: Handle the case where the payment result was not
                //       persisted.
                continue;
            }

            let entry = swaps
                .entry(redeemable.swap.public.hash)
                .or_insert(Vec::new());
            entry.push(redeemable.clone());
        }

        let mut futures: FuturesUnordered<RedeemFut> = FuturesUnordered::new();
        for (redeem, redeemables) in recheck_redeems {
            let fut = self.recheck_redeem(current_height, redeem.clone(), redeemables.clone());
            futures.push(Box::pin(async move {
                let res = fut.await;
                (res, Some(redeem), redeemables)
            }));
        }

        for (_, redeemables) in swaps {
            let fut = self.redeem(current_height, redeemables.clone());

            futures.push(Box::pin(async move {
                let res = fut.await;
                (res, None, redeemables)
            }));
        }

        while let Some((res, redeem, redeemables)) = futures.next().await {
            // TODO: Extract the reason for error, and come up with solutions
            //       how to redo the redeeming another way.
            if let Err(e) = res {
                let redeemables = redeemables
                    .iter()
                    .map(|redeemable| redeemable.utxo.outpoint.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                match redeem {
                    Some(redeem) => error!(
                        "failed to recheck redeem '{}' for outpoints '{}': {:?}",
                        redeem.tx.txid(),
                        redeemables,
                        e
                    ),
                    None => error!(
                        "failed to create redeem for outpoints '{}': {:?}",
                        redeemables, e
                    ),
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    async fn recheck_redeem(
        &self,
        current_height: u64,
        redeem: Redeem,
        redeemables: Vec<RedeemableUtxo>,
    ) -> Result<(), RedeemError> {
        let blocks_left = match redeemables
            .iter()
            .map(|r| r.blocks_left(current_height))
            .min()
        {
            Some(blocks_left) => blocks_left,
            None => return Err(RedeemError::General("blocks_left returned none".into())),
        };
        let fee_estimate = self.fee_estimator.estimate_fee(blocks_left).await?;

        // If the feerate is still sufficient, rebroadcast the same transaction.
        if redeem.fee_per_kw + MIN_REPLACEMENT_DIFF_SAT_PER_KW > fee_estimate.sat_per_kw {
            return match self.chain_client.broadcast_tx(redeem.tx.clone()).await {
                Ok(_) => {
                    debug!("succesfully rebroadcast redeem tx '{}'", redeem.tx.txid());
                    Ok(())
                }
                Err(e) => match e {
                    BroadcastError::Chain(_) => Err(e.into()),
                    BroadcastError::InsufficientFeeRejectingReplacement(_) => {
                        debug!(
                            "rebroadcast redeem tx '{}' returned expected error '{}'",
                            redeem.tx.txid(),
                            e
                        );
                        Ok(())
                    }
                    BroadcastError::UnknownError(_) => Err(e.into()),
                },
            };
        }

        // The fee rate is not sufficient, craft a replacement transaction.
        let replacement = self
            .redeem_service
            .redeem(
                &redeemables,
                &fee_estimate,
                current_height,
                redeem.destination_address,
                redeem.auto_bump,
            )
            .await?;
        debug!(
            tx_id = field::display(replacement.txid()),
            prev_tx_id = field::display(redeem.tx.txid()),
            "broadcasted replacement redeem tx"
        );
        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    async fn redeem(
        &self,
        current_height: u64,
        redeemables: Vec<RedeemableUtxo>,
    ) -> Result<(), RedeemError> {
        let blocks_left = match redeemables
            .iter()
            .map(|r| r.blocks_left(current_height))
            .min()
        {
            Some(blocks_left) => blocks_left,
            None => return Err(RedeemError::General("blocks_left returned none".into())),
        };
        let fee_estimate_fut = self.fee_estimator.estimate_fee(blocks_left);
        let address_fut = self.wallet.new_address();
        let (fee_estimate_res, address_res) = join!(fee_estimate_fut, address_fut);
        let fee_estimate = fee_estimate_res?;
        let destination_address = address_res?;

        // Craft a redeem transaction
        self.redeem_service
            .redeem(
                &redeemables,
                &fee_estimate,
                current_height,
                destination_address,
                true,
            )
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
