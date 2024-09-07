use std::{collections::HashMap, time::SystemTime};

use bitcoin::{hashes::sha256, secp256k1, Address};
use thiserror::Error;

use crate::chain::Utxo;

use super::{swap_service::Swap, SwapState};

#[derive(Debug)]
pub enum SwapPersistenceError {
    AlreadyExists,
    General(Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub enum AddPreimageError {
    DoesNotExist,
    General(Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub enum GetSwapError {
    NotFound,
    InvalidPreimage,
    General(Box<dyn std::error::Error>),
}

#[derive(Debug, Error)]
pub enum GetSwapsError {
    #[error("invalid preimage")]
    InvalidPreimage,
    #[error("{0}")]
    General(Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub struct PaymentAttempt {
    pub creation_time: SystemTime,
    pub payment_hash: sha256::Hash,
    pub utxos: Vec<Utxo>,
    pub amount_msat: u64,
    pub destination: secp256k1::PublicKey,
    pub payment_request: String,
}

#[async_trait::async_trait]
pub trait SwapRepository {
    async fn add_swap(&self, swap: &Swap) -> Result<(), SwapPersistenceError>;
    async fn add_payment_attempt(
        &self,
        attempt: &PaymentAttempt,
    ) -> Result<(), SwapPersistenceError>;
    async fn add_preimage(&self, swap: &Swap, preimage: &[u8; 32]) -> Result<(), AddPreimageError>;
    async fn get_swap(&self, hash: &sha256::Hash) -> Result<SwapState, GetSwapError>;
    async fn get_swaps(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, SwapState>, GetSwapsError>;
}
