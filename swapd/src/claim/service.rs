use std::{sync::Arc, time::SystemTime};

use bitcoin::{Address, Transaction};
use thiserror::Error;
use tracing::{debug, field};

use crate::{
    chain::{ChainClient, ChainRepository, ChainRepositoryError, FeeEstimate},
    claim::Claim,
    swap::{ClaimableUtxo, GetSwapsError, PrivateKeyProvider, SwapRepository, SwapService},
};

use super::ClaimRepository;

#[derive(Debug, Error)]
pub enum ClaimServiceError {
    #[error("claim service: {0}")]
    ChainRepository(ChainRepositoryError),
    #[error("claim service: {0}")]
    GetSwaps(GetSwapsError),
}

#[derive(Debug, Error)]
pub enum ClaimError {
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Debug)]
pub struct ClaimService<CC, CR, RR, SR, P>
where
    CC: ChainClient,
    CR: ChainRepository,
    RR: ClaimRepository,
    SR: SwapRepository,
    P: PrivateKeyProvider,
{
    chain_client: Arc<CC>,
    chain_repository: Arc<CR>,
    claim_repository: Arc<RR>,
    swap_repository: Arc<SR>,
    swap_service: Arc<SwapService<P>>,
}

impl<CC, CR, RR, SR, P> ClaimService<CC, CR, RR, SR, P>
where
    CC: ChainClient,
    CR: ChainRepository,
    RR: ClaimRepository,
    SR: SwapRepository,
    P: PrivateKeyProvider,
{
    pub fn new(
        chain_client: Arc<CC>,
        chain_repository: Arc<CR>,
        claim_repository: Arc<RR>,
        swap_repository: Arc<SR>,
        swap_service: Arc<SwapService<P>>,
    ) -> Self {
        Self {
            chain_client,
            chain_repository,
            claim_repository,
            swap_repository,
            swap_service,
        }
    }

    pub async fn list_claimable(&self) -> Result<Vec<ClaimableUtxo>, ClaimServiceError> {
        let utxos = self.chain_repository.get_utxos().await?;
        let addresses: Vec<_> = utxos.iter().map(|u| u.address.clone()).collect();
        let swaps = self
            .swap_repository
            .get_swaps_with_paid_outpoints(&addresses)
            .await?;
        let mut claimable_utxos = Vec::new();
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
            claimable_utxos.push(ClaimableUtxo {
                utxo: utxo.utxo,
                paid_with_request: paid_with_invoice,
                preimage,
                swap: swap.swap_state.swap.clone(),
            });
        }

        Ok(claimable_utxos)
    }

    pub async fn claim(
        &self,
        claimables: &[ClaimableUtxo],
        fee_estimate: &FeeEstimate,
        current_height: u64,
        destination_address: Address,
        auto_bump: bool,
    ) -> Result<Transaction, ClaimError> {
        let tx = self.swap_service.create_claim_tx(
            claimables,
            fee_estimate,
            current_height,
            destination_address.clone(),
        )?;

        let outpoints: Vec<_> = claimables
            .iter()
            .map(|r| r.utxo.outpoint.to_string())
            .collect();
        debug!(
            fee_per_kw = fee_estimate.sat_per_kw,
            outpoints = field::debug(outpoints),
            tx_id = field::display(tx.compute_txid()),
            "broadcasting claim tx"
        );
        self.chain_client.broadcast_tx(tx.clone()).await?;
        self.claim_repository
            .add_claim(&Claim {
                creation_time: SystemTime::now(),
                destination_address: destination_address.clone(),
                fee_per_kw: fee_estimate.sat_per_kw,
                tx: tx.clone(),
                auto_bump,
            })
            .await?;
        self.chain_repository
            .add_watch_address(&destination_address)
            .await?;
        Ok(tx)
    }
}

impl From<ChainRepositoryError> for ClaimServiceError {
    fn from(value: ChainRepositoryError) -> Self {
        ClaimServiceError::ChainRepository(value)
    }
}

impl From<GetSwapsError> for ClaimServiceError {
    fn from(value: GetSwapsError) -> Self {
        ClaimServiceError::GetSwaps(value)
    }
}
