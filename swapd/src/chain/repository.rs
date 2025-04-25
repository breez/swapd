use bitcoin::{Address, BlockHash, OutPoint, Txid};
use thiserror::Error;

use super::types::{BlockHeader, Txo, TxoWithSpend};

#[derive(Debug, Error)]
pub enum ChainRepositoryError {
    #[error("multiple tips")]
    MultipleTips,
    #[error("{0}")]
    General(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub struct AddressUtxo {
    pub address: Address,
    pub utxo: Txo,
}

#[derive(Debug)]
pub struct SpentTxo {
    pub outpoint: OutPoint,
    pub spending_tx: Txid,
    pub spending_input_index: u32,
}

#[async_trait::async_trait]
pub trait ChainRepository {
    async fn add_block(
        &self,
        block: &BlockHeader,
        tx_outputs: &[AddressUtxo],
        tx_inputs: &[SpentTxo],
    ) -> Result<Vec<SpentTxo>, ChainRepositoryError>;
    async fn add_watch_address(&self, address: &Address) -> Result<(), ChainRepositoryError>;
    async fn filter_watch_addresses(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Address>, ChainRepositoryError>;
    async fn get_block_headers(&self) -> Result<Vec<BlockHeader>, ChainRepositoryError>;
    async fn get_tip(&self) -> Result<Option<BlockHeader>, ChainRepositoryError>;
    async fn get_txos_for_address(
        &self,
        address: &Address,
    ) -> Result<Vec<Txo>, ChainRepositoryError>;
    async fn get_txos_for_address_with_spends(
        &self,
        address: &Address,
    ) -> Result<Vec<TxoWithSpend>, ChainRepositoryError>;
    async fn get_utxos(&self) -> Result<Vec<AddressUtxo>, ChainRepositoryError>;
    async fn undo_block(&self, hash: BlockHash) -> Result<(), ChainRepositoryError>;
}
