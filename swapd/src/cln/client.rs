use tonic::{transport::Uri, Request};
use tracing::{debug, error, instrument, warn};

use crate::lightning::{LightningClient, PayError};

use super::cln_api::{node_client::NodeClient, pay_response::PayStatus, PayRequest};

#[derive(Debug)]
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
    #[instrument(level = "trace", skip(self))]
    async fn pay(&self, bolt11: String) -> Result<[u8; 32], PayError> {
        let mut client = match NodeClient::connect(self.address.clone()).await {
            Ok(client) => client,
            Err(e) => {
                error!("failed to connect to cln: {:?}", e);
                return Err(e.into());
            }
        };
        let resp = match client
            .pay(Request::new(PayRequest {
                bolt11,
                ..Default::default()
            }))
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                debug!("failed to pay: {:?}", e);
                return Err(e.into());
            }
        }
        .into_inner();
        let preimage = match resp.status() {
            PayStatus::Complete => resp.payment_preimage.try_into().map_err(|e| {
                warn!("failed to parse preimage from cln: {:?}", e);
                PayError::InvalidPreimage
            })?,
            PayStatus::Pending => todo!(),
            PayStatus::Failed => todo!(),
        };
        Ok(preimage)
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
