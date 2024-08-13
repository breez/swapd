use tonic::{transport::Uri, Request};

use crate::{
    chain::{ChainClient, ChainError},
    lightning::{LightningClient, PayError},
};

use super::cln_api::{node_client::NodeClient, pay_response::PayStatus, PayRequest};

pub struct Client {
    address: Uri,
}

impl Client {
    pub fn new(address: Uri) -> Self {
        Self { address }
    }
}

#[async_trait::async_trait]
impl LightningClient for Client {
    async fn pay(&self, bolt11: String) -> Result<[u8; 32], PayError> {
        let mut client = NodeClient::connect(self.address.clone()).await?;
        let resp = client
            .pay(Request::new(PayRequest {
                bolt11,
                ..Default::default()
            }))
            .await?
            .into_inner();
        let preimage = match resp.status() {
            PayStatus::Complete => resp
                .payment_preimage
                .try_into()
                .map_err(|_| PayError::InvalidPreimage)?,
            PayStatus::Pending => todo!(),
            PayStatus::Failed => todo!(),
        };
        Ok(preimage)
    }
}

#[async_trait::async_trait]
impl ChainClient for Client {
    async fn get_blockheight(&self) -> Result<u32, ChainError> {
        todo!()
    }
}

impl From<tonic::transport::Error> for PayError {
    fn from(_value: tonic::transport::Error) -> Self {
        PayError::ConnectionFailed
    }
}

impl From<tonic::Status> for PayError {
    fn from(_value: tonic::Status) -> Self {
        PayError::ConnectionFailed
    }
}
