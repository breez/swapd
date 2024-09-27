use bitcoin::hashes::sha256;
use tonic::Status;

#[derive(Debug)]
pub enum LightningError {
    ConnectionFailed,
    InvalidPreimage,
    General(Status),
}

#[derive(Debug)]
pub struct PaymentRequest {
    pub bolt11: String,
    pub payment_hash: sha256::Hash,
    pub label: String,
}

#[derive(Debug)]
pub enum PaymentResult {
    Success { preimage: [u8; 32] },
    Failure { error: String },
}

#[derive(Debug)]
pub struct PreimageResult {
    pub preimage: [u8; 32],
    pub label: String,
}

#[async_trait::async_trait]
pub trait LightningClient {
    async fn get_preimage(
        &self,
        hash: sha256::Hash,
    ) -> Result<Option<PreimageResult>, LightningError>;
    async fn pay(&self, request: PaymentRequest) -> Result<PaymentResult, LightningError>;
}
