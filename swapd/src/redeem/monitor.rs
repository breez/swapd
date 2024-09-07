use std::{collections::HashMap, future::Future, sync::Arc};

use crate::{
    chain::{ChainRepository, ChainRepositoryError, FeeEstimator},
    swap::{GetSwapsError, PrivateKeyProvider, SwapRepository, SwapService},
};

#[derive(Debug)]
pub enum RedeemError {
    General(Box<dyn std::error::Error>)
}
pub struct RedeemMonitor<CR, FE, SR, P>
where
    CR: ChainRepository,
    FE: FeeEstimator,
    SR: SwapRepository,
    P: PrivateKeyProvider,
{
    chain_repository: Arc<CR>,
    fee_estimator: Arc<FE>,
    swap_repository: Arc<SR>,
    swap_service: Arc<SwapService<P>>,
}

impl<CR, FE, SR, P> RedeemMonitor<CR, FE, SR, P>
where
    CR: ChainRepository,
    FE: FeeEstimator,
    SR: SwapRepository,
    P: PrivateKeyProvider,
{
    pub fn new(
        chain_repository: Arc<CR>,
        fee_estimator: Arc<FE>,
        swap_repository: Arc<SR>,
        swap_service: Arc<SwapService<P>>,
    ) -> Self {
        Self {
            chain_repository,
            fee_estimator,
            swap_repository,
            swap_service,
        }
    }

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
                    .or_insert((swap, Vec::new()));
                entry.1.push(utxo);
            }

            // TODO: get the right utxos for each swap.
        }
    }
}

impl From<ChainRepositoryError> for RedeemError {
    fn from(value: ChainRepositoryError) -> Self {
        match value {
            ChainRepositoryError::General(e) => RedeemError::General(e),
        }
    }
}

impl From<GetSwapsError> for RedeemError {
    fn from(value: GetSwapsError) -> Self {
        RedeemError::General(Box::new(value))
    }
}