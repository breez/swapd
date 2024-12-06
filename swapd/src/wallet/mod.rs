use bitcoin::Address;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WalletError {
    #[error("creation failed")]
    CreationFailed,
    #[error("invalid address: {0}")]
    InvalidAddress(bitcoin::address::ParseError),
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[async_trait::async_trait]
pub trait Wallet {
    async fn new_address(&self) -> Result<Address, WalletError>;
}
