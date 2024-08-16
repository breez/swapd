use bitcoin::Address;

#[async_trait::async_trait]
pub trait ChainFilterRepository {
    async fn has_filtered_address(
        &self,
        addresses: &[Address],
    ) -> Result<bool, Box<dyn std::error::Error>>;
}
