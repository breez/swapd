use std::collections::HashMap;

use bitcoin::{Address, BlockHash, OutPoint, Txid};
use thiserror::Error;

use super::types::{BlockHeader, Utxo};

#[derive(Debug, Error)]
pub enum ChainRepositoryError {
    #[error("{0}")]
    General(Box<dyn std::error::Error + Send + Sync>),
}

pub struct AddressUtxo {
    pub address: Address,
    pub utxo: Utxo,
}

pub struct SpentUtxo {
    pub outpoint: OutPoint,
    pub spending_tx: Txid,
    pub spending_block: BlockHash,
}

#[async_trait::async_trait]
pub trait ChainRepository {
    async fn add_block(&self, block: &BlockHeader) -> Result<(), ChainRepositoryError>;
    async fn add_watch_address(&self, address: &Address) -> Result<(), ChainRepositoryError>;
    async fn add_watch_addresses(&self, addresses: &[Address]) -> Result<(), ChainRepositoryError>;
    async fn add_utxo(&self, utxo: &AddressUtxo) -> Result<(), ChainRepositoryError>;
    async fn add_utxos(&self, utxos: &[AddressUtxo]) -> Result<(), ChainRepositoryError>;
    async fn filter_watch_addresses(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Address>, ChainRepositoryError>;
    async fn get_block_headers(&self) -> Result<Vec<BlockHeader>, ChainRepositoryError>;
    async fn get_utxos_for_address(
        &self,
        address: &Address,
    ) -> Result<Vec<Utxo>, ChainRepositoryError>;
    async fn get_utxos_for_addresses(
        &self,
        address: &[Address],
    ) -> Result<HashMap<Address, Vec<Utxo>>, ChainRepositoryError>;
    async fn mark_spent(&self, utxos: &[SpentUtxo]) -> Result<(), ChainRepositoryError>;
    async fn undo_block(&self, hash: BlockHash) -> Result<(), ChainRepositoryError>;
}
