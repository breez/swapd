use std::time::SystemTime;

use bitcoin::{Address, OutPoint, Transaction};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Redeem {
    pub creation_time: SystemTime,
    pub tx: Transaction,
    pub destination_address: Address,
    pub fee_per_kw: u32,
    pub auto_bump: bool,
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

    /// Get all redeems where the inputs haven't been spent yet, sorted by fee
    /// rate desc, then creation time desc.
    async fn get_redeems(
        &self,
        outpoints: &[OutPoint],
    ) -> Result<Vec<Redeem>, RedeemRepositoryError>;
}
