use bitcoin::Address;
use tracing::instrument;

use crate::chain_filter;

#[derive(Debug)]
pub struct ChainFilterRepository {}

impl ChainFilterRepository {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl chain_filter::ChainFilterRepository for ChainFilterRepository {
    #[instrument(level = "trace", skip(self))]
    async fn has_filtered_address(
        &self,
        addresses: &[Address],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        todo!()
    }
}
