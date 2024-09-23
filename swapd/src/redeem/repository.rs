use std::time::SystemTime;

use bitcoin::{hashes::sha256, Address, Transaction};
use thiserror::Error;

#[derive(Debug)]
pub struct Redeem {
    pub swap_hash: sha256::Hash,
    pub creation_time: SystemTime,
    pub tx: Transaction,
    pub destination_address: Address,
    pub fee_per_kw: u32,
}

#[derive(Debug, Error)]
pub enum RedeemRepositoryError {
    #[error("invalid timestamp")]
    InvalidTimestamp,
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[async_trait::async_trait]
pub trait RedeemRepository {
    async fn add_redeem(&self, redeem: &Redeem) -> Result<(), RedeemRepositoryError>;
    async fn get_last_redeem(
        &self,
        swap_hash: &sha256::Hash,
    ) -> Result<Option<Redeem>, RedeemRepositoryError>;
}
