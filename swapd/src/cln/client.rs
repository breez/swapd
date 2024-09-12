use tonic::{transport::Uri, Request};
use tracing::{debug, error, instrument, warn};

use crate::lightning::{LightningClient, PayError, PaymentResult};

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
    async fn pay(&self, label: String, bolt11: String) -> Result<PaymentResult, PayError> {
        let mut client = match NodeClient::connect(self.address.clone()).await {
            Ok(client) => client,
            Err(e) => {
                error!("failed to connect to cln: {:?}", e);
                return Err(e.into());
            }
        };
        // TODO: Properly map the response here.
        let pay_resp = match client
            .pay(Request::new(PayRequest {
                label: Some(label),
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
        let resp = match pay_resp.status() {
            PayStatus::Complete => {
                let preimage = pay_resp.payment_preimage.try_into().map_err(|e| {
                    warn!("failed to parse preimage from cln: {:?}", e);
                    PayError::InvalidPreimage
                })?;
                PaymentResult::Success { preimage }
              },
            PayStatus::Pending => todo!(),
            PayStatus::Failed => todo!(),
        };
        Ok(resp)
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
