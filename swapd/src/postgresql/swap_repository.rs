use bitcoin::{hashes::sha256, Address};
use tracing::instrument;

use crate::server::{self, AddressState, GetSwapError, Swap, SwapPersistenceError, SwapState};

#[derive(Debug)]
pub struct SwapRepository {}

impl SwapRepository {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl server::SwapRepository for SwapRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_swap(&self, _swap: &Swap) -> Result<(), SwapPersistenceError> {
        todo!()
    }

    #[instrument(level = "trace", skip(self))]
    async fn add_preimage(
        &self,
        _swap: &Swap,
        _preimage: &[u8; 32],
    ) -> Result<(), Box<dyn std::error::Error>> {
        todo!()
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_state_by_hash(
        &self,
        _hash: &sha256::Hash,
    ) -> Result<SwapState, GetSwapError> {
        todo!()
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_state(
        &self,
        _addresses: Vec<Address>,
    ) -> Result<Vec<AddressState>, Box<dyn std::error::Error>> {
        todo!()
    }
}
