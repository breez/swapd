use bitcoin::hashes::sha256;
use thiserror::Error;
use tonic::Status;

#[derive(Debug, Error)]
pub enum LightningError {
    #[error("connection failed")]
    ConnectionFailed,
    #[error("invalid preimage")]
    InvalidPreimage,
    #[error("payment not found")]
    PaymentNotFound,
    #[error("general: {0}")]
    General(Status),
}

#[derive(Debug)]
pub struct PaymentRequest {
    pub bolt11: String,
    pub cltv_limit: u32,
    pub payment_hash: sha256::Hash,
    pub label: String,
    pub fee_limit_msat: u64,
    pub timeout_seconds: u16,
}

#[derive(Debug)]
pub enum PaymentResult {
    Success { preimage: [u8; 32] },
    Failure { error: String },
}

#[derive(Debug)]
pub enum PaymentState {
    Success { preimage: [u8; 32] },
    Failure { error: String },
    Pending,
}

#[derive(Debug)]
pub struct PreimageResult {
    pub preimage: [u8; 32],
    pub label: String,
}

#[async_trait::async_trait]
pub trait LightningClient {
    async fn get_payment_state(
        &self,
        hash: sha256::Hash,
        label: &str,
    ) -> Result<PaymentState, LightningError>;
    async fn get_preimage(
        &self,
        hash: sha256::Hash,
    ) -> Result<Option<PreimageResult>, LightningError>;
    async fn has_pending_or_complete_payment(
        &self,
        hash: &sha256::Hash,
    ) -> Result<bool, LightningError>;
    async fn pay(&self, request: PaymentRequest) -> Result<PaymentResult, LightningError>;
}
