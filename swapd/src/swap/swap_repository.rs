use std::{collections::HashMap, time::SystemTime};

use bitcoin::{hashes::sha256, secp256k1, Address, OutPoint};
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum LockSwapError {
    #[error("already locked")]
    AlreadyLocked,
    #[error("swap not found")]
    SwapNotFound,
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

#[derive(Debug, Error)]
pub enum GetUnhandledPaymentAttemptsError {
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[derive(Clone, Debug)]
pub struct PaymentAttempt {
    pub creation_time: SystemTime,
    pub label: String,
    pub payment_hash: sha256::Hash,
    pub outputs: Vec<OutPoint>,
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
    async fn get_unhandled_payment_attempts(
        &self,
    ) -> Result<Vec<PaymentAttempt>, GetUnhandledPaymentAttemptsError>;
    async fn lock_add_payment_attempt(
        &self,
        swap: &Swap,
        attempt: &PaymentAttempt,
    ) -> Result<(), LockSwapError>;
    async fn lock_swap_refund(&self, swap: &Swap, refund_id: &str) -> Result<(), LockSwapError>;
    async fn unlock_add_payment_result(
        &self,
        swap: &Swap,
        payment_label: &str,
        result: &PaymentResult,
    ) -> Result<(), LockSwapError>;
    async fn unlock_swap_refund(&self, swap: &Swap, refund_id: &str) -> Result<(), LockSwapError>;
}
