use std::{sync::Arc, time::SystemTime};

use bitcoin::{Address, Transaction};
use thiserror::Error;
use tracing::{debug, field};

use crate::{
    chain::{ChainClient, ChainRepository, ChainRepositoryError, FeeEstimate},
    redeem::Redeem,
    swap::{GetSwapsError, PrivateKeyProvider, RedeemableUtxo, SwapRepository, SwapService},
};

use super::RedeemRepository;

#[derive(Debug, Error)]
pub enum RedeemServiceError {
    #[error("redeem service: {0}")]
    ChainRepository(ChainRepositoryError),
    #[error("redeem service: {0}")]
    GetSwaps(GetSwapsError),
}

#[derive(Debug, Error)]
pub enum RedeemError {
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Debug)]
pub struct RedeemService<CC, CR, RR, SR, P>
where
    CC: ChainClient,
    CR: ChainRepository,
    RR: RedeemRepository,
    SR: SwapRepository,
    P: PrivateKeyProvider,
{
    chain_client: Arc<CC>,
    chain_repository: Arc<CR>,
    redeem_repository: Arc<RR>,
    swap_repository: Arc<SR>,
    swap_service: Arc<SwapService<P>>,
}

impl<CC, CR, RR, SR, P> RedeemService<CC, CR, RR, SR, P>
where
    CC: ChainClient,
    CR: ChainRepository,
    RR: RedeemRepository,
    SR: SwapRepository,
    P: PrivateKeyProvider,
{
    pub fn new(
        chain_client: Arc<CC>,
        chain_repository: Arc<CR>,
        redeem_repository: Arc<RR>,
        swap_repository: Arc<SR>,
        swap_service: Arc<SwapService<P>>,
    ) -> Self {
        Self {
            chain_client,
            chain_repository,
            redeem_repository,
            swap_repository,
            swap_service,
        }
    }

    pub async fn list_redeemable(&self) -> Result<Vec<RedeemableUtxo>, RedeemServiceError> {
        let utxos = self.chain_repository.get_utxos().await?;
        let addresses: Vec<_> = utxos.iter().map(|u| u.address.clone()).collect();
        let swaps = self
            .swap_repository
            .get_swaps_with_paid_outpoints(&addresses)
            .await?;
        let mut redeemable_utxos = Vec::new();
        for utxo in utxos {
            let swap = match swaps.get(&utxo.address) {
                Some(swap) => swap,
                None => continue,
            };

            let preimage = match swap.swap_state.preimage {
                Some(preimage) => preimage,
                None => continue,
            };

            let paid_with_invoice = swap
                .paid_outpoints
                .iter()
                .find(|po| po.outpoint == utxo.utxo.outpoint)
                .map(|po| po.payment_request.clone());
            redeemable_utxos.push(RedeemableUtxo {
                utxo: utxo.utxo,
                paid_with_request: paid_with_invoice,
                preimage,
                swap: swap.swap_state.swap.clone(),
            });
        }

        Ok(redeemable_utxos)
    }

    pub async fn redeem(
        &self,
        redeemables: &[RedeemableUtxo],
        fee_estimate: &FeeEstimate,
        current_height: u64,
        destination_address: Address,
        auto_bump: bool,
    ) -> Result<Transaction, RedeemError> {
        let tx = self.swap_service.create_redeem_tx(
            redeemables,
            fee_estimate,
            current_height,
            destination_address.clone(),
        )?;

        let outpoints: Vec<_> = redeemables
            .iter()
            .map(|r| r.utxo.outpoint.to_string())
            .collect();
        debug!(
            fee_per_kw = fee_estimate.sat_per_kw,
            outpoints = field::debug(outpoints),
            tx_id = field::display(tx.txid()),
            "broadcasting redeem tx"
        );
        self.chain_client.broadcast_tx(tx.clone()).await?;
        self.redeem_repository
            .add_redeem(&Redeem {
                creation_time: SystemTime::now(),
                destination_address,
                fee_per_kw: fee_estimate.sat_per_kw,
                tx: tx.clone(),
                auto_bump,
            })
            .await?;
        Ok(tx)
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
