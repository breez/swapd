use bitcoin::{Address, Block, BlockHash, OutPoint};
use thiserror::Error;

use super::ChainRepositoryError;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("{0}")]
    Database(ChainRepositoryError),
    #[error("empty chain")]
    EmptyChain,
    #[error("invalid chain")]
    InvalidChain,
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[async_trait::async_trait]
pub trait ChainClient {
    async fn get_blockheight(&self) -> Result<u64, ChainError>;
    async fn get_tip_hash(&self) -> Result<BlockHash, ChainError>;
    async fn get_block(&self, hash: &BlockHash) -> Result<Block, ChainError>;
    async fn get_block_header(
        &self,
        hash: &BlockHash,
    ) -> Result<super::types::BlockHeader, ChainError>;
    async fn get_sender_addresses(&self, utxos: &[OutPoint]) -> Result<Vec<Address>, ChainError>;
}
