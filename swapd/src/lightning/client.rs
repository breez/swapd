#[derive(Debug)]
pub enum PayError {
    ConnectionFailed,
    InvalidPreimage,
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
    async fn pay(&self, label: String, bolt11: String) -> Result<PaymentResult, PayError>;
}
