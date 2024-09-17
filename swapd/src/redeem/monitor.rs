use std::{collections::HashMap, future::Future, sync::Arc, time::SystemTime};

use futures::{stream::FuturesUnordered, StreamExt};
use tokio::join;
use tracing::{error, field, warn};

use crate::{
    chain::{
        ChainClient, ChainError, ChainRepository, ChainRepositoryError, FeeEstimateError,
        FeeEstimator,
    },
    swap::{CreateRedeemTxError, GetSwapsError, PrivateKeyProvider, SwapRepository, SwapService},
    wallet::{Wallet, WalletError},
};

use super::{repository::RedeemRepository, Redeem, RedeemRepositoryError};

#[derive(Debug)]
pub enum RedeemError {
    General(Box<dyn std::error::Error>),
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
    pub fn new(
        chain_client: Arc<CC>,
        chain_repository: Arc<CR>,
        fee_estimator: Arc<FE>,
        swap_repository: Arc<SR>,
        swap_service: Arc<SwapService<P>>,
        redeem_repository: Arc<RR>,
        wallet: Arc<W>,
    ) -> Self {
        Self {
            chain_client,
            chain_repository,
            fee_estimator,
            swap_repository,
            swap_service,
            redeem_repository,
            wallet,
        }
    }

    // TODO: use the stop signal
    // TODO: add intervals to the loop
    // TODO: add proper error handling
    pub async fn start<F: Future<Output = ()>>(&self, signal: F) -> Result<(), RedeemError> {
        loop {
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

            let mut utxo_tasks = FuturesUnordered::new();
            for (_, (swap, preimage, utxos)) in redeemable_swaps {
                let swap_repository = Arc::clone(&self.swap_repository);
                utxo_tasks.push(async move {
                    // TODO: Remove any outpoints that no longer exist in the utxos list.
                    // TODO: It is perhaps better to save the utxos we tried to redeem in a transaction.
                    //       That way we avoid pinning ourselves due to RBF'ing with potentially different input sets. 
                    let utxos: Vec<_> = match swap_repository.get_paid_outpoints(&swap.public.hash).await {
                        Ok(outpoints) => if outpoints.is_empty() {
                            warn!(
                                hash=field::display(swap.public.hash),
                                "Could not find paid outpoints for paid swap, redeeming all known utxos");

                            // If the outpoint list is empty, claim all utxos to be sure to redeem something.
                            utxos
                        } else {
                            // Take only outputs that are still unspent. If some are skipped, that may be a loss.
                            utxos.into_iter().filter(|u|outpoints.contains(&u.outpoint)).collect()
                        },
                        Err(e) => {
                            error!(
                                hash=field::display(swap.public.hash),
                                "Failed to get paid outpoints for paid swap, redeeming all known utxos: {:?}", e);

                            // If the database call failed, claim all utxos to be sure to redeem something.
                            utxos
                        }
                    };
                    (swap, preimage, utxos)
                });
            }

            let current_height = self.chain_client.get_blockheight().await?;

            let mut redeem_tasks = FuturesUnordered::new();
            while let Some((swap, preimage, utxos)) = utxo_tasks.next().await {
                let fee_estimator = Arc::clone(&self.fee_estimator);
                let wallet = Arc::clone(&self.wallet);
                let swap_service = Arc::clone(&self.swap_service);
                let chain_client = Arc::clone(&self.chain_client);
                let redeem_repository = Arc::clone(&self.redeem_repository);
                let min_conf_height = match utxos.iter().map(|u| u.block_height).min() {
                    Some(min_conf_height) => min_conf_height,
                    None => continue,
                };
                let blocks_left = (swap.public.lock_time as i32)
                    - (current_height.saturating_sub(min_conf_height) as i32);
                redeem_tasks.push(async move {
                    let fee_estimate_fut = fee_estimator.estimate_fee(blocks_left);
                    let address_fut = wallet.new_address();
                    let (fee_estimate_res, address_res) = join!(fee_estimate_fut, address_fut);
                    let fee_estimate = fee_estimate_res?;
                    let destination_address = address_res?;
                    let redeem_tx = swap_service.create_redeem_tx(
                        &swap,
                        &utxos,
                        &fee_estimate,
                        current_height,
                        &preimage,
                        destination_address.clone(),
                    )?;
                    redeem_repository
                        .add_redeem(&Redeem {
                            creation_time: SystemTime::now(),
                            destination_address,
                            fee_per_kw: fee_estimate.sat_per_kw,
                            tx: redeem_tx.clone(),
                        })
                        .await?;
                    chain_client.broadcast_tx(redeem_tx).await?;
                    Ok::<(), RedeemError>(())
                });
            }

            while let Some(result) = redeem_tasks.next().await {
                // TODO: Handle result
            }
        }
    }
}

impl From<bitcoin::address::Error> for RedeemError {
    fn from(value: bitcoin::address::Error) -> Self {
        todo!()
    }
}

impl From<ChainRepositoryError> for RedeemError {
    fn from(value: ChainRepositoryError) -> Self {
        match value {
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
        }
    }
}

impl From<CreateRedeemTxError> for RedeemError {
    fn from(value: CreateRedeemTxError) -> Self {
        todo!()
    }
}

impl From<FeeEstimateError> for RedeemError {
    fn from(value: FeeEstimateError) -> Self {
        todo!()
    }
}

impl From<GetSwapsError> for RedeemError {
    fn from(value: GetSwapsError) -> Self {
        RedeemError::General(Box::new(value))
    }
}

impl From<RedeemRepositoryError> for RedeemError {
    fn from(value: RedeemRepositoryError) -> Self {
        todo!()
    }
}

impl From<WalletError> for RedeemError {
    fn from(value: WalletError) -> Self {
        todo!()
    }
}
