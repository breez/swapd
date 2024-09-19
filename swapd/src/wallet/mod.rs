use bitcoin::Address;

#[derive(Debug)]
pub enum WalletError {
    CreationFailed,
    InvalidAddress(bitcoin::address::Error),
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[async_trait::async_trait]
pub trait Wallet {
    async fn new_address(&self) -> Result<Address, WalletError>;
}
