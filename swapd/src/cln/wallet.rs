use bitcoin::{address::NetworkUnchecked, Address};
use tonic::Request;

use crate::{
    cln::cln_api::NewaddrRequest,
    wallet::{Wallet, WalletError},
};

use super::{client::GetClientError, Client};

#[async_trait::async_trait]
impl Wallet for Client {
    async fn new_address(&self) -> Result<Address, WalletError> {
        let mut client = self.get_client().await?;

        let addr_resp = client
            .new_addr(Request::new(NewaddrRequest {
                ..Default::default()
            }))
            .await?
            .into_inner();

        let address = match addr_resp.bech32.or(addr_resp.p2tr) {
            Some(address) => address,
            None => return Err(WalletError::CreationFailed),
        };

        let address: Address<NetworkUnchecked> = address.parse()?;
        let address = address.require_network(self.network)?;
        Ok(address)
    }
}

impl From<GetClientError> for WalletError {
    fn from(value: GetClientError) -> Self {
        match value {
            GetClientError::ConnectionFailed(_) => WalletError::General(Box::new(value)),
        }
    }
}

impl From<tonic::Status> for WalletError {
    fn from(value: tonic::Status) -> Self {
        WalletError::General(Box::new(value))
    }
}

impl From<bitcoin::address::Error> for WalletError {
    fn from(value: bitcoin::address::Error) -> Self {
        WalletError::InvalidAddress(value)
    }
}
