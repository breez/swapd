use bitcoin::{Address, OutPoint};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Utxo {
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