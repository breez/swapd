#[derive(Debug)]
pub enum PayError {}

#[async_trait::async_trait]
pub trait LightningClient {
    async fn pay(&self, bolt11: String) -> Result<[u8; 32], PayError>;
}
