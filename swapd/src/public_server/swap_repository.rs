use bitcoin::{hashes::sha256, Address, Txid};

use crate::chain::Utxo;

use super::swap_service::Swap;

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
    General(Box<dyn std::error::Error>),
}

pub struct AddressState {
    pub address: Address,
    pub status: AddressStatus,
}

pub enum AddressStatus {
    Unknown,
    Mempool {
        tx_info: TxInfo,
    },
    Confirmed {
        block_hash: sha256::Hash,
        block_height: u64,
        tx_info: TxInfo,
    },
}

pub struct TxInfo {
    pub tx: Txid,
    pub amount: u64,
}

pub struct SwapState {
    pub swap: Swap,
    pub utxos: Vec<Utxo>,
}

#[async_trait::async_trait]
pub trait SwapRepository {
    async fn add_swap(&self, swap: &Swap) -> Result<(), SwapPersistenceError>;
    async fn add_preimage(&self, swap: &Swap, preimage: &[u8; 32]) -> Result<(), AddPreimageError>;
    async fn get_swap_state_by_hash(&self, hash: &sha256::Hash) -> Result<SwapState, GetSwapError>;
    async fn get_state(
        &self,
        addresses: Vec<Address>,
    ) -> Result<Vec<AddressState>, Box<dyn std::error::Error>>;
}
