use std::collections::HashMap;

use bitcoin::{hashes::sha256, Address};

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

#[derive(Debug)]
pub enum GetSwapsError {
    InvalidPreimage,
    General(Box<dyn std::error::Error>),
}

#[async_trait::async_trait]
pub trait SwapRepository {
    async fn add_swap(&self, swap: &Swap) -> Result<(), SwapPersistenceError>;
    async fn add_preimage(&self, swap: &Swap, preimage: &[u8; 32]) -> Result<(), AddPreimageError>;
    async fn get_swap(&self, hash: &sha256::Hash) -> Result<SwapState, GetSwapError>;
    async fn get_swaps(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, SwapState>, GetSwapsError>;
}
