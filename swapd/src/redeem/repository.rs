use std::time::SystemTime;

use bitcoin::{Address, Transaction};

#[derive(Debug)]
pub struct Redeem {
    pub creation_time: SystemTime,
    pub tx: Transaction,
    pub destination_address: Address,
    pub fee_per_kw: u32,
}

#[derive(Debug)]
pub enum RedeemRepositoryError {
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[async_trait::async_trait]
pub trait RedeemRepository {
    async fn add_redeem(&self, redeem: &Redeem) -> Result<(), RedeemRepositoryError>;
}
