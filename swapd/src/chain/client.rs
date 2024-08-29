use bitcoin::{hashes::sha256, Address, OutPoint};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Utxo {
    pub block_hash: sha256::Hash,
    pub block_height: u32,
    pub outpoint: OutPoint,
    pub amount_sat: u64,
}

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("{0}")]
    General(Box<dyn std::error::Error>),
}

#[async_trait::async_trait]
pub trait ChainClient {
    async fn get_blockheight(&self) -> Result<u32, ChainError>;
    async fn get_sender_addresses(&self, utxos: &[OutPoint]) -> Result<Vec<Address>, ChainError>;
}

impl From<Box<dyn std::error::Error>> for ChainError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        ChainError::General(value)
    }
}
