use bitcoin::{
    hashes::{sha256, Hash},
    Network,
};
use futures::{stream::FuturesUnordered, StreamExt};
use regex::Regex;
use thiserror::Error;
use tokio::join;
use tonic::{
    transport::{Certificate, Channel, ClientTlsConfig, Identity, Uri},
    Request, Status,
};
use tracing::{debug, error, instrument, warn};

use crate::lightning::{
    LightningClient, LightningError, PaymentRequest, PaymentResult, PreimageResult,
};

use super::cln_api::{
    listsendpays_request::ListsendpaysStatus, node_client::NodeClient, pay_response::PayStatus,
    Amount, ListpaysRequest, ListsendpaysRequest, PayRequest, WaitsendpayRequest,
};

pub struct ClientConnection {
    pub address: Uri,
    pub ca_cert: Certificate,
    pub identity: Identity,
}

#[derive(Debug)]
pub struct Client {
    pub(super) network: Network,
    address: Uri,
    tls_config: ClientTlsConfig,
}

impl Client {
    pub fn new(connection: ClientConnection, network: Network) -> Self {
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(connection.ca_cert)
            .identity(connection.identity);
        Self {
            address: connection.address,
            network,
            tls_config,
        }
    }

    pub(super) async fn get_client(&self) -> Result<NodeClient<Channel>, GetClientError> {
        let channel = match Channel::builder(self.address.clone())
            .tls_config(self.tls_config.clone())?
            .connect()
            .await
        {
            Ok(channel) => channel,
            Err(e) => {
                error!("failed to connect to cln: {:?}", e);
                return Err(e.into());
            }
        };

        Ok(NodeClient::new(channel))
    }
}

#[async_trait::async_trait]
impl LightningClient for Client {
    #[instrument(level = "trace", skip(self))]
    async fn get_preimage(
        &self,
        hash: sha256::Hash,
    ) -> Result<Option<PreimageResult>, LightningError> {
        let mut client = self.get_client().await?;
        let resp = client
            .list_pays(Request::new(ListpaysRequest {
                payment_hash: Some(hash.as_byte_array().to_vec()),
                ..Default::default()
            }))
            .await?;

        let result = match resp
            .into_inner()
            .pays
            .into_iter()
            .find(|pay| pay.preimage.is_some())
        {
            Some(payment) => Some(PreimageResult {
                label: String::from(payment.label()),
                preimage: payment.preimage.unwrap().try_into().map_err(|e| {
                    warn!("failed to parse preimage from cln: {:?}", e);
                    LightningError::InvalidPreimage
                })?,
            }),
            None => None,
        };

        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn pay(&self, request: PaymentRequest) -> Result<PaymentResult, LightningError> {
        let mut client = self.get_client().await?;

        let pay_resp = match client
            .pay(Request::new(PayRequest {
                label: Some(request.label),
                bolt11: request.bolt11,
                maxfee: Some(Amount {
                    msat: request.fee_limit_msat,
                }),
                retry_for: Some(request.timeout_seconds as u32),
                maxdelay: Some(request.cltv_limit),
                ..Default::default()
            }))
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                debug!("pay returned error {:?}", e);
                return match wait_payment(&mut client, request.payment_hash).await? {
                    Some(preimage) => Ok(preimage),
                    None => Ok(PaymentResult::Failure {
                        error: "unknown failure".to_string(),
                    }),
                };
            }
        }
        .into_inner();
        let resp = match pay_resp.status() {
            PayStatus::Complete => {
                let preimage = pay_resp.payment_preimage.try_into().map_err(|e| {
                    warn!("failed to parse preimage from cln: {:?}", e);
                    LightningError::InvalidPreimage
                })?;
                PaymentResult::Success { preimage }
            }
            PayStatus::Pending => {
                warn!("payment is pending after pay returned");
                return match wait_payment(&mut client, request.payment_hash).await? {
                    Some(result) => Ok(result),
                    None => Ok(PaymentResult::Failure {
                        error: "unknown failure".to_string(),
                    }),
                };
            }
            PayStatus::Failed => {
                if let Some(warning) = pay_resp.warning_partial_completion {
                    warn!("pay returned partial completion: {}", warning);
                    return match wait_payment(&mut client, request.payment_hash).await? {
                        Some(result) => Ok(result),
                        None => Ok(PaymentResult::Failure {
                            error: "unknown failure".to_string(),
                        }),
                    };
                };
                return Ok(PaymentResult::Failure {
                    error: "unknown failure".to_string(),
                });
            }
        };
        Ok(resp)
    }
}

async fn wait_payment<'a>(
    client: &mut NodeClient<Channel>,
    payment_hash: sha256::Hash,
) -> Result<Option<PaymentResult>, LightningError> {
    let mut client2 = client.clone();
    let completed_payments_fut = client.list_send_pays(Request::new(ListsendpaysRequest {
        payment_hash: Some(payment_hash.as_byte_array().to_vec()),
        bolt11: None,
        index: None,
        limit: None,
        start: None,
        status: Some(ListsendpaysStatus::Complete.into()),
    }));
    let pending_payments_fut = client2.list_send_pays(Request::new(ListsendpaysRequest {
        payment_hash: Some(payment_hash.as_byte_array().to_vec()),
        bolt11: None,
        index: None,
        limit: None,
        start: None,
        status: Some(ListsendpaysStatus::Pending.into()),
    }));
    let (completed_payments, pending_payments) =
        join!(completed_payments_fut, pending_payments_fut);
    let (completed_payments, pending_payments) = (completed_payments?, pending_payments?);

    if let Some(preimage) = completed_payments
        .into_inner()
        .payments
        .into_iter()
        .filter_map(|p| p.payment_preimage)
        .next()
    {
        return Ok(Some(PaymentResult::Success {
            preimage: preimage
                .try_into()
                .map_err(|_| LightningError::InvalidPreimage)?,
        }));
    }

    let mut tasks = FuturesUnordered::new();
    for payment in pending_payments.into_inner().payments {
        let mut client = client.clone();
        tasks.push(async move {
            client
                .wait_send_pay(Request::new(WaitsendpayRequest {
                    groupid: Some(payment.groupid),
                    partid: payment.partid,
                    payment_hash: payment_hash.as_byte_array().to_vec(),
                    timeout: None,
                }))
                .await
        });
    }

    while let Some(res) = tasks.next().await {
        match res {
            Ok(res) => {
                if let Some(preimage) = res.into_inner().payment_preimage {
                    return Ok(Some(PaymentResult::Success {
                        preimage: preimage
                            .try_into()
                            .map_err(|_| LightningError::InvalidPreimage)?,
                    }));
                }
            }
            // TODO: Map these errors to correct error strings.
            Err(status) => match parse_cln_error(&status) {
                Some(code) => match code {
                    -1 => return Err(LightningError::General(status)),
                    200 => return Err(LightningError::General(status)),
                    202 => {}
                    203 => {}
                    204 => {}
                    208 => {}
                    209 => {}
                    _ => return Err(LightningError::General(status)),
                },
                None => return Err(LightningError::General(status)),
            },
        }
    }

    Ok(None)
}

fn parse_cln_error(status: &Status) -> Option<i32> {
    let re: Regex = Regex::new(r"Some\((?<code>-?\d+)\)").unwrap();
    re.captures(status.message())
        .and_then(|caps| caps["code"].parse::<i32>().ok())
}

impl From<tonic::transport::Error> for LightningError {
    fn from(_value: tonic::transport::Error) -> Self {
        LightningError::ConnectionFailed
    }
}

impl From<tonic::Status> for LightningError {
    fn from(_value: tonic::Status) -> Self {
        LightningError::ConnectionFailed
    }
}

#[derive(Debug, Error)]
pub(super) enum GetClientError {
    #[error("connection failed")]
    ConnectionFailed(Box<dyn std::error::Error + Sync + Send>),
}

impl From<tonic::transport::Error> for GetClientError {
    fn from(value: tonic::transport::Error) -> Self {
        GetClientError::ConnectionFailed(Box::new(value))
    }
}

impl From<GetClientError> for LightningError {
    fn from(value: GetClientError) -> Self {
        match value {
            GetClientError::ConnectionFailed(_) => LightningError::ConnectionFailed,
        }
    }
}
