use std::{collections::HashMap, sync::Arc};

use thiserror::Error;

use crate::{
    chain::{ChainRepository, ChainRepositoryError, Utxo},
    swap::{GetSwapsError, Swap, SwapRepository},
};

#[derive(Debug)]
pub struct Redeemable {
    pub swap: Swap,
    pub utxos: Vec<RedeemableUtxo>,
    pub preimage: [u8; 32],
}

#[derive(Debug)]
pub struct RedeemableUtxo {
    pub utxo: Utxo,
    pub paid_with_request: Option<String>,
}

impl Redeemable {
    pub fn blocks_left(&self, current_height: u64) -> i32 {
        let min_conf_height = self
            .utxos
            .iter()
            .map(|u| u.utxo.block_height)
            .min()
            .unwrap_or(0);
        (self.swap.public.lock_time as i32)
            - (current_height.saturating_sub(min_conf_height) as i32)
    }
}

#[derive(Debug, Error)]
pub enum RedeemServiceError {
    #[error("redeem service: {0}")]
    ChainRepository(ChainRepositoryError),
    #[error("redeem service: {0}")]
    GetSwaps(GetSwapsError),
}

#[derive(Debug)]
pub struct RedeemService<CR, SR>
where
    CR: ChainRepository,
    SR: SwapRepository,
{
    chain_repository: Arc<CR>,
    swap_repository: Arc<SR>,
}

impl<CR, SR> RedeemService<CR, SR>
where
    CR: ChainRepository,
    SR: SwapRepository,
{
    pub fn new(chain_repository: Arc<CR>, swap_repository: Arc<SR>) -> Self {
        Self {
            chain_repository,
            swap_repository,
        }
    }

    pub async fn list_redeemable(&self) -> Result<Vec<Redeemable>, RedeemServiceError> {
        let utxos = self.chain_repository.get_utxos().await?;
        let addresses: Vec<_> = utxos.iter().map(|u| u.address.clone()).collect();
        let swaps = self
            .swap_repository
            .get_swaps_with_paid_outpoints(&addresses)
            .await?;
        let mut redeemable_swaps = HashMap::new();
        for utxo in utxos {
            let swap = match swaps.get(&utxo.address) {
                Some(swap) => swap,
                None => continue,
            };

            let preimage = match swap.swap_state.preimage {
                Some(preimage) => preimage,
                None => continue,
            };

            let entry = redeemable_swaps
                .entry(swap.swap_state.swap.public.address.clone())
                .or_insert((swap.swap_state.swap.clone(), preimage, Vec::new()));
            let paid_with_invoice = swap
                .paid_outpoints
                .iter()
                .find(|po| po.outpoint == utxo.utxo.outpoint)
                .map(|po| po.payment_request.clone());
            entry.2.push(RedeemableUtxo {
                utxo: utxo.utxo,
                paid_with_request: paid_with_invoice,
            });
        }

        Ok(redeemable_swaps
            .into_iter()
            .filter(|(_, (_, _, utxos))| !utxos.is_empty())
            .map(|(_, (swap, preimage, utxos))| Redeemable {
                preimage,
                swap,
                utxos,
            })
            .collect())
    }
}

impl From<ChainRepositoryError> for RedeemServiceError {
    fn from(value: ChainRepositoryError) -> Self {
        RedeemServiceError::ChainRepository(value)
    }
}

impl From<GetSwapsError> for RedeemServiceError {
    fn from(value: GetSwapsError) -> Self {
        RedeemServiceError::GetSwaps(value)
    }
}
