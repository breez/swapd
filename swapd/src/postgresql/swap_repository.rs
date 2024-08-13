use bitcoin::{hashes::sha256, Address};

use crate::server::{self, AddressState, GetSwapError, Swap, SwapPersistenceError, SwapState};

pub struct SwapRepository {}

impl SwapRepository {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl server::SwapRepository for SwapRepository {
    async fn add_swap(&self, _swap: &Swap) -> Result<(), SwapPersistenceError> {
        todo!()
    }
    async fn add_preimage(
        &self,
        _swap: &Swap,
        _preimage: &[u8; 32],
    ) -> Result<(), Box<dyn std::error::Error>> {
        todo!()
    }
    async fn get_swap_state_by_hash(
        &self,
        _hash: &sha256::Hash,
    ) -> Result<SwapState, GetSwapError> {
        todo!()
    }
    async fn get_state(
        &self,
        _addresses: Vec<Address>,
    ) -> Result<Vec<AddressState>, Box<dyn std::error::Error>> {
        todo!()
    }
}
