use std::time::Duration;
use std::{collections::HashMap, future::Future, pin::pin, sync::Arc, time::SystemTime};

use futures::future::{FusedFuture, FutureExt};
use futures::{stream::FuturesUnordered, StreamExt};
use thiserror::Error;
use tokio::join;
use tracing::{debug, error, field, warn};

use crate::chain::Utxo;
use crate::swap::Swap;
use crate::{
    chain::{
        ChainClient, ChainError, ChainRepository, ChainRepositoryError, FeeEstimateError,
        FeeEstimator,
    },
    swap::{CreateRedeemTxError, GetSwapsError, PrivateKeyProvider, SwapRepository, SwapService},
    wallet::{Wallet, WalletError},
};

use super::{repository::RedeemRepository, Redeem, RedeemRepositoryError};

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
    pub chain_repository: Arc<CR>,
    pub fee_estimator: Arc<FE>,
    pub poll_interval: Duration,
    pub swap_repository: Arc<SR>,
    pub swap_service: Arc<SwapService<P>>,
    pub redeem_repository: Arc<RR>,
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
    chain_repository: Arc<CR>,
    fee_estimator: Arc<FE>,
    poll_interval: Duration,
    swap_repository: Arc<SR>,
    swap_service: Arc<SwapService<P>>,
    redeem_repository: Arc<RR>,
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
            chain_repository: params.chain_repository,
            fee_estimator: params.fee_estimator,
            poll_interval: params.poll_interval,
            swap_repository: params.swap_repository,
            swap_service: params.swap_service,
            redeem_repository: params.redeem_repository,
            wallet: params.wallet,
        }
    }

    // TODO: use the stop signal
    // TODO: add intervals to the loop
    // TODO: add proper error handling
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

    async fn do_sync(&self) -> Result<(), RedeemError> {
        let utxos = self.chain_repository.get_utxos().await?;
        let addresses: Vec<_> = utxos.iter().map(|u| u.address.clone()).collect();
        let swaps = self.swap_repository.get_swaps(&addresses).await?;
        let mut redeemable_swaps = HashMap::new();
        for utxo in utxos {
            let swap = match swaps.get(&utxo.address) {
                Some(swap) => swap,
                None => continue,
            };

            let preimage = match swap.preimage {
                Some(preimage) => preimage,
                None => continue,
            };

            let entry = redeemable_swaps
                .entry(swap.swap.public.address.clone())
                .or_insert((swap.swap.clone(), preimage, Vec::new()));
            entry.2.push(utxo.utxo);
        }

        let current_height = self.chain_client.get_blockheight().await?;

        let mut tasks = FuturesUnordered::new();
        for (_, (swap, preimage, utxos)) in redeemable_swaps {
            tasks.push(self.redeem_swap(swap, preimage, utxos, current_height))
        }

        while let Some(result) = tasks.next().await {
            if let Err(e) = result {
                // TODO: Log which task it was.
                error!("redeem task errored with: {:?}", e);
            }
        }

        Ok(())
    }

    async fn redeem_swap(
        &self,
        swap: Swap,
        preimage: [u8; 32],
        utxos: Vec<Utxo>,
        current_height: u64,
    ) -> Result<(), RedeemError> {
        // TODO: Remove any outpoints that no longer exist in the utxos list.
        // TODO: It is perhaps better to save the utxos we tried to redeem in a transaction.
        //       That way we avoid pinning ourselves due to RBF'ing with potentially different input sets.
        let utxos: Vec<_> = match self
            .swap_repository
            .get_paid_outpoints(&swap.public.hash)
            .await
        {
            Ok(outpoints) => {
                if outpoints.is_empty() {
                    warn!(
                        hash = field::display(swap.public.hash),
                        "Could not find paid outpoints for paid swap, redeeming all known utxos"
                    );

                    // If the outpoint list is empty, claim all utxos to be sure to redeem something.
                    utxos
                } else {
                    // Take only outputs that are still unspent. If some are skipped, that may be a loss.
                    utxos
                        .into_iter()
                        .filter(|u| outpoints.contains(&u.outpoint))
                        .collect()
                }
            }
            Err(e) => {
                error!(
                    hash = field::display(swap.public.hash),
                    "Failed to get paid outpoints for paid swap, redeeming all known utxos: {:?}",
                    e
                );

                // If the database call failed, claim all utxos to be sure to redeem something.
                utxos
            }
        };

        if utxos.is_empty() {
            return Ok(());
        }

        // NOTE: This unwrap only works because the utxos vec is not empty!
        let min_conf_height = utxos.iter().map(|u| u.block_height).min().unwrap();
        let blocks_left = (swap.public.lock_time as i32)
            - (current_height.saturating_sub(min_conf_height) as i32);
        let fee_estimate_fut = self.fee_estimator.estimate_fee(blocks_left);
        let address_fut = self.wallet.new_address();
        let (fee_estimate_res, address_res) = join!(fee_estimate_fut, address_fut);
        let fee_estimate = fee_estimate_res?;
        let destination_address = address_res?;
        // TODO: Rebroadcast the old tx if the fee is sufficient.
        let redeem_tx = self.swap_service.create_redeem_tx(
            &swap,
            &utxos,
            &fee_estimate,
            current_height,
            &preimage,
            destination_address.clone(),
        )?;
        self.redeem_repository
            .add_redeem(&Redeem {
                creation_time: SystemTime::now(),
                destination_address,
                fee_per_kw: fee_estimate.sat_per_kw,
                tx: redeem_tx.clone(),
            })
            .await?;
        self.chain_client.broadcast_tx(redeem_tx).await?;
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
