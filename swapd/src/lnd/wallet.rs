use bitcoin::{address::NetworkUnchecked, Address};

use crate::wallet::{Wallet, WalletError};

use super::{client::GetClientError, lnrpc::NewAddressRequest, Client, Repository};

impl From<GetClientError> for WalletError {
    fn from(value: GetClientError) -> Self {
        match value {
            GetClientError::ConnectionFailed(_) => WalletError::General(Box::new(value)),
        }
    }
}

#[async_trait::async_trait]
impl<R> Wallet for Client<R>
where
    R: Repository + Send + Sync,
{
    async fn new_address(&self) -> Result<Address, WalletError> {
        let mut client = self.get_client().await?;
        let resp = client
            .new_address(NewAddressRequest {
                ..Default::default()
            })
            .await?
            .into_inner();
        let address: Address<NetworkUnchecked> = resp.address.parse()?;
        let address = address.require_network(self.network)?;
        Ok(address)
    }
}
