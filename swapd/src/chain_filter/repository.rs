use bitcoin::Address;

#[async_trait::async_trait]
pub trait ChainFilterRepository {
    async fn add_filter_addresses(
        &self,
        addresses: &[Address],
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn has_filtered_address(
        &self,
        addresses: &[Address],
    ) -> Result<bool, Box<dyn std::error::Error>>;
}
