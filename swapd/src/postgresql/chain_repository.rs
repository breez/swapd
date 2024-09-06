use std::sync::Arc;

use bitcoin::{Address, BlockHash};
use sqlx::PgPool;

use crate::chain::{self, AddressUtxo, BlockHeader, ChainRepositoryError, SpentUtxo};

#[derive(Debug)]
pub struct ChainRepository {
    pool: Arc<PgPool>,
}

impl ChainRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl chain::ChainRepository for ChainRepository {
    async fn add_block(&self, block: &BlockHeader) -> Result<(), ChainRepositoryError> {
        todo!()
    }
    async fn add_watch_address(&self, address: Address) -> Result<(), ChainRepositoryError> {
        todo!()
    }
    async fn add_watch_addresses(&self, address: &[Address]) -> Result<(), ChainRepositoryError> {
        todo!()
    }
    async fn add_utxo(&self, utxo: &AddressUtxo) -> Result<(), ChainRepositoryError> {
        todo!()
    }
    async fn add_utxos(&self, utxos: &[AddressUtxo]) -> Result<(), ChainRepositoryError> {
        todo!()
    }
    async fn filter_watch_addresses(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Address>, ChainRepositoryError> {
        todo!()
    }
    async fn get_block_headers(&self) -> Result<Vec<BlockHeader>, ChainRepositoryError> {
        todo!()
    }
    async fn mark_spent(&self, utxos: &[SpentUtxo]) -> Result<(), ChainRepositoryError> {
        todo!()
    }
    async fn undo_block(&self, hash: BlockHash) -> Result<(), ChainRepositoryError> {
        todo!()
    }
}
