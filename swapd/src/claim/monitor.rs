use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::time::Duration;
use std::{future::Future, sync::Arc};

use bitcoin::OutPoint;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::join;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, field, instrument};

use crate::chain::BroadcastError;
use crate::swap::ClaimableUtxo;
use crate::{
    chain::{
        ChainClient, ChainError, ChainRepository, ChainRepositoryError, FeeEstimateError,
        FeeEstimator,
    },
    swap::{CreateClaimTxError, GetSwapsError, PrivateKeyProvider, SwapRepository},
    wallet::{Wallet, WalletError},
};

use super::service::{ClaimService, ClaimServiceError};
use super::ClaimError;
use super::{repository::ClaimRepository, Claim, ClaimRepositoryError};

const MIN_REPLACEMENT_DIFF_SAT_PER_KW: u32 = 250;
type ClaimFut<'a> = Pin<
    Box<
        dyn Future<Output = (Result<(), ClaimError>, Option<Claim>, Vec<ClaimableUtxo>)>
            + Send
            + 'a,
    >,
>;

pub struct ClaimMonitorParams<CC, CR, FE, SR, P, RR, W>
where
    CC: ChainClient,
    CR: ChainRepository,
    FE: FeeEstimator,
    SR: SwapRepository,
    P: PrivateKeyProvider,
    RR: ClaimRepository,
    W: Wallet,
{
    pub chain_client: Arc<CC>,
    pub fee_estimator: Arc<FE>,
    pub poll_interval: Duration,
    pub claim_repository: Arc<RR>,
    pub claim_service: Arc<ClaimService<CC, CR, RR, SR, P>>,
    pub wallet: Arc<W>,
}

pub struct ClaimMonitor<CC, CR, FE, SR, P, RR, W>
where
    CC: ChainClient,
    CR: ChainRepository,
    FE: FeeEstimator,
    SR: SwapRepository,
    P: PrivateKeyProvider,
    RR: ClaimRepository,
    W: Wallet,
{
    chain_client: Arc<CC>,
    fee_estimator: Arc<FE>,
    poll_interval: Duration,
    claim_repository: Arc<RR>,
    claim_service: Arc<ClaimService<CC, CR, RR, SR, P>>,
    wallet: Arc<W>,
}

impl<CC, CR, FE, SR, P, RR, W> ClaimMonitor<CC, CR, FE, SR, P, RR, W>
where
    CC: ChainClient + Send + Sync,
    CR: ChainRepository + Send + Sync,
    FE: FeeEstimator + Send + Sync,
    SR: SwapRepository + Send + Sync,
    P: PrivateKeyProvider + Send + Sync,
    RR: ClaimRepository + Send + Sync,
    W: Wallet + Send + Sync,
{
    pub fn new(params: ClaimMonitorParams<CC, CR, FE, SR, P, RR, W>) -> Self {
        Self {
            chain_client: params.chain_client,
            fee_estimator: params.fee_estimator,
            poll_interval: params.poll_interval,
            claim_repository: params.claim_repository,
            claim_service: params.claim_service,
            wallet: params.wallet,
        }
    }

    pub async fn start(&self, token: CancellationToken) -> Result<(), ClaimError> {
        loop {
            if token.is_cancelled() {
                return Ok(());
            }

            debug!("starting claim task");
            match self.do_claim().await {
                Ok(_) => debug!("claim task completed succesfully"),
                Err(e) => error!("claim task failed with: {:?}", e),
            }

            tokio::select! {
                _ = token.cancelled() => {
                    debug!("claim monitor shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {}
            }
        }

        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    async fn do_claim(&self) -> Result<(), ClaimError> {
        let current_height = self.chain_client.get_blockheight().await?;
        let claimables = self.claim_service.list_claimable().await?;
        let claimables: HashMap<_, _> = claimables
            .into_iter()
            .map(|claimable| (claimable.utxo.outpoint, claimable))
            .collect();
        let outpoints: Vec<_> = claimables.keys().cloned().collect();

        // Get existing claims, sorted by highest fee rate and then creation time.
        let claims = self.claim_repository.get_claims(&outpoints).await?;

        // First remove all the outpoints where there is already an in-progress
        // claim transaction published. These in-progress claim transactions
        // will be rechecked for fees below.
        let mut recheck_claims = Vec::new();
        let mut unhandled_outpoints: HashSet<OutPoint> = outpoints.iter().cloned().collect();
        for claim in claims {
            let outpoints: Vec<_> = claim
                .tx
                .input
                .iter()
                .map(|input| input.previous_output)
                .collect();
            // Only reprocess this claim if it is still spending valid outputs.
            if !outpoints
                .iter()
                .all(|outpoint| claimables.contains_key(outpoint))
            {
                continue;
            }

            let mut current_claimables: Vec<ClaimableUtxo> = Vec::new();
            for outpoint in &outpoints {
                unhandled_outpoints.remove(outpoint);
                current_claimables.push(
                    claimables
                        .get(outpoint)
                        .expect("missing expected claimable utxo in map")
                        .clone(),
                );
            }

            recheck_claims.push((claim, current_claimables));
        }

        // Now group the remaining utxos by swap. Note grouping by swap is
        // pretty much an arbitrary decision. they might as well be grouped by
        // remaining timelock to save on fees.
        let mut swaps = HashMap::new();
        for unhandled_outpoint in &unhandled_outpoints {
            let claimable = claimables
                .get(unhandled_outpoint)
                .expect("missing expected claimable utxo in map");

            // Be a good citizen and don't claim any funds that were not paid
            // over lightning. That can happen if the user sends another onchain
            // transaction to the same address multiple times. Users may not
            // know this is unsafe for them to do.
            if claimable.paid_with_request.is_none() {
                // TODO: Handle the case where the payment result was not
                //       persisted.
                continue;
            }

            let entry = swaps
                .entry(claimable.swap.public.hash)
                .or_insert(Vec::new());
            entry.push(claimable.clone());
        }

        let mut futures: FuturesUnordered<ClaimFut> = FuturesUnordered::new();
        for (claim, claimables) in recheck_claims {
            let fut = self.recheck_claim(current_height, claim.clone(), claimables.clone());
            futures.push(Box::pin(async move {
                let res = fut.await;
                (res, Some(claim), claimables)
            }));
        }

        for (_, claimables) in swaps {
            let fut = self.claim(current_height, claimables.clone());

            futures.push(Box::pin(async move {
                let res = fut.await;
                (res, None, claimables)
            }));
        }

        while let Some((res, claim, claimables)) = futures.next().await {
            // TODO: Extract the reason for error, and come up with solutions
            //       how to redo the claiming another way.
            if let Err(e) = res {
                let claimables = claimables
                    .iter()
                    .map(|claimable| claimable.utxo.outpoint.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                match claim {
                    Some(claim) => error!(
                        tx_id = field::display(claim.tx.compute_txid()),
                        outpoints = claimables,
                        "failed to recheck claim: {:?}",
                        e
                    ),
                    None => error!(outpoints = claimables, "failed to create claim: {:?}", e),
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    async fn recheck_claim(
        &self,
        current_height: u64,
        claim: Claim,
        claimables: Vec<ClaimableUtxo>,
    ) -> Result<(), ClaimError> {
        let blocks_left = match claimables
            .iter()
            .map(|r| r.swap.blocks_left(current_height))
            .min()
        {
            Some(blocks_left) => blocks_left,
            None => return Err(ClaimError::General("blocks_left returned none".into())),
        };
        let fee_estimate = self.fee_estimator.estimate_fee(blocks_left).await?;

        let claim_txid = claim.tx.compute_txid();
        // If the feerate is still sufficient, rebroadcast the same transaction.
        if claim.fee_per_kw + MIN_REPLACEMENT_DIFF_SAT_PER_KW > fee_estimate.sat_per_kw {
            return match self.chain_client.broadcast_tx(claim.tx.clone()).await {
                Ok(_) => {
                    debug!("succesfully rebroadcast claim tx '{}'", claim_txid);
                    Ok(())
                }
                Err(e) => match e {
                    BroadcastError::Chain(_) => Err(e.into()),
                    BroadcastError::InsufficientFeeRejectingReplacement(_) => {
                        debug!(
                            "rebroadcast claim tx '{}' returned expected error '{}'",
                            claim_txid, e
                        );
                        Ok(())
                    }
                    BroadcastError::UnknownError(_) => Err(e.into()),
                },
            };
        }

        // The fee rate is not sufficient, craft a replacement transaction.
        let replacement = self
            .claim_service
            .claim(
                &claimables,
                &fee_estimate,
                current_height,
                claim.destination_address,
                claim.auto_bump,
            )
            .await?;
        debug!(
            tx_id = field::display(replacement.compute_txid()),
            prev_tx_id = field::display(claim_txid),
            "broadcasted replacement claim tx"
        );
        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    async fn claim(
        &self,
        current_height: u64,
        claimables: Vec<ClaimableUtxo>,
    ) -> Result<(), ClaimError> {
        let blocks_left = match claimables
            .iter()
            .map(|r| r.swap.blocks_left(current_height))
            .min()
        {
            Some(blocks_left) => blocks_left,
            None => return Err(ClaimError::General("blocks_left returned none".into())),
        };
        let fee_estimate_fut = self.fee_estimator.estimate_fee(blocks_left);
        let address_fut = self.wallet.new_address();
        let (fee_estimate_res, address_res) = join!(fee_estimate_fut, address_fut);
        let fee_estimate = fee_estimate_res?;
        let destination_address = address_res?;

        // Craft a claim transaction
        self.claim_service
            .claim(
                &claimables,
                &fee_estimate,
                current_height,
                destination_address,
                true,
            )
            .await?;
        Ok(())
    }
}

impl From<ChainRepositoryError> for ClaimError {
    fn from(value: ChainRepositoryError) -> Self {
        match value {
            ChainRepositoryError::MultipleTips => ClaimError::General(Box::new(value)),
            ChainRepositoryError::General(e) => ClaimError::General(e),
        }
    }
}

impl From<ChainError> for ClaimError {
    fn from(value: ChainError) -> Self {
        match value {
            ChainError::General(e) => ClaimError::General(e),
            ChainError::Database(_) => ClaimError::General(Box::new(value)),
            ChainError::EmptyChain => ClaimError::General(Box::new(value)),
            ChainError::InvalidChain => ClaimError::General(Box::new(value)),
            ChainError::BlockNotFound => ClaimError::General(Box::new(value)),
        }
    }
}

impl From<BroadcastError> for ClaimError {
    fn from(value: BroadcastError) -> Self {
        ClaimError::General(Box::new(value))
    }
}

impl From<CreateClaimTxError> for ClaimError {
    fn from(value: CreateClaimTxError) -> Self {
        ClaimError::General(Box::new(value))
    }
}

impl From<FeeEstimateError> for ClaimError {
    fn from(value: FeeEstimateError) -> Self {
        ClaimError::General(Box::new(value))
    }
}

impl From<GetSwapsError> for ClaimError {
    fn from(value: GetSwapsError) -> Self {
        ClaimError::General(Box::new(value))
    }
}

impl From<ClaimRepositoryError> for ClaimError {
    fn from(value: ClaimRepositoryError) -> Self {
        ClaimError::General(Box::new(value))
    }
}

impl From<WalletError> for ClaimError {
    fn from(value: WalletError) -> Self {
        ClaimError::General(Box::new(value))
    }
}

impl From<ClaimServiceError> for ClaimError {
    fn from(value: ClaimServiceError) -> Self {
        ClaimError::General(Box::new(value))
    }
}
