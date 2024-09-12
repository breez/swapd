use bitcoin::hashes::sha256;
use tonic::Status;

#[derive(Debug)]
pub enum PayError {
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
    Success {
        preimage: [u8; 32],
    },
    Failure {
        error: String,
    }
}

#[async_trait::async_trait]
pub trait LightningClient {
    async fn pay(&self, request: PaymentRequest) -> Result<PaymentResult, PayError>;
}
