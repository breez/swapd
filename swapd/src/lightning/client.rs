use bitcoin::hashes::sha256;
use lightning_invoice::Bolt11Invoice;
use tonic::Status;

#[derive(Debug)]
pub enum LightningError {
    ConnectionFailed,
    InvalidPreimage,
    NoRoute,
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
pub struct PreimageResult {
    pub preimage: [u8; 32],
    pub label: String,
}

#[derive(Debug)]
pub struct Route {
    // The total delay over the route.
    pub delay: u32,
}

#[async_trait::async_trait]
pub trait LightningClient {
    async fn get_preimage(
        &self,
        hash: sha256::Hash,
    ) -> Result<Option<PreimageResult>, LightningError>;
    async fn get_route(&self, bolt11: &Bolt11Invoice) -> Result<Route, LightningError>;
    async fn has_pending_or_complete_payment(
        &self,
        hash: &sha256::Hash,
    ) -> Result<bool, LightningError>;
    async fn pay(&self, request: PaymentRequest) -> Result<PaymentResult, LightningError>;
}
