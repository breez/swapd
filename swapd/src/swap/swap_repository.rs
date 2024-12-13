use std::{collections::HashMap, time::SystemTime};

use bitcoin::{hashes::sha256, secp256k1, Address, OutPoint};
use thiserror::Error;

use crate::chain::Utxo;
use crate::lightning::PaymentResult;

use super::{swap_service::Swap, SwapState};

#[derive(Debug)]
pub enum SwapPersistenceError {
    AlreadyExists,
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Debug, Error)]
pub enum AddPaymentResultError {
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum LockType {
    Pay,
    Refund,
}

impl TryFrom<i32> for LockType {
    type Error = LockSwapError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => LockType::Pay,
            1 => LockType::Refund,
            _ => return Err(LockSwapError::InvalidLockType),
        })
    }
}

impl From<LockType> for i32 {
    fn from(value: LockType) -> i32 {
        match value {
            LockType::Pay => 0,
            LockType::Refund => 1,
        }
    }
}

#[derive(Debug, Error)]
pub enum LockSwapError {
    #[error("already locked")]
    AlreadyLocked,
    #[error("invalid lock type")]
    InvalidLockType,
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Debug, Error)]
pub enum GetPaidUtxosError {
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Debug, Error)]
pub enum GetSwapsError {
    #[error("swap not found")]
    NotFound,
    #[error("invalid preimage")]
    InvalidPreimage,
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Debug)]
pub struct PaymentAttempt {
    pub creation_time: SystemTime,
    pub label: String,
    pub payment_hash: sha256::Hash,
    pub utxos: Vec<Utxo>,
    pub amount_msat: u64,
    pub destination: secp256k1::PublicKey,
    pub payment_request: String,
}

#[derive(Debug)]
pub struct SwapStatePaidOutpoints {
    pub swap_state: SwapState,
    pub paid_outpoints: Vec<PaidOutpoint>,
}

#[derive(Debug)]
pub struct PaidOutpoint {
    pub outpoint: OutPoint,
    pub payment_request: String,
}

#[async_trait::async_trait]
pub trait SwapRepository {
    async fn add_swap(&self, swap: &Swap) -> Result<(), SwapPersistenceError>;
    async fn add_payment_attempt(
        &self,
        attempt: &PaymentAttempt,
    ) -> Result<(), SwapPersistenceError>;
    async fn add_payment_result(
        &self,
        hash: &sha256::Hash,
        label: &str,
        result: &PaymentResult,
    ) -> Result<(), AddPaymentResultError>;
    async fn get_swap_by_hash(&self, hash: &sha256::Hash) -> Result<SwapState, GetSwapsError>;
    async fn get_swap_by_address(&self, address: &Address) -> Result<SwapState, GetSwapsError>;
    async fn get_swap_by_payment_request(
        &self,
        payment_request: &str,
    ) -> Result<SwapState, GetSwapsError>;
    async fn get_swaps(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, SwapState>, GetSwapsError>;
    async fn get_swaps_with_paid_outpoints(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, SwapStatePaidOutpoints>, GetSwapsError>;
    async fn lock_swap(
        &self,
        swap: &Swap,
        lock_type: LockType,
    ) -> Result<Option<LockType>, LockSwapError>;
    async fn unlock_swap(&self, swap: &Swap) -> Result<(), LockSwapError>;
}
