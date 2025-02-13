use std::sync::Arc;

use bitcoin::{
    hashes::{sha256, Hash},
    Network,
};
use thiserror::Error;
use tonic::{
    metadata::{errors::InvalidMetadataValue, Ascii, MetadataValue},
    service::interceptor::InterceptedService,
    transport::{Certificate, Channel, ClientTlsConfig, Uri},
    Request, Status,
};
use tracing::{error, field, instrument, trace, warn};

use crate::{
    lightning::{LightningError, PaymentResult, PaymentState, PreimageResult},
    lnd::routerrpc::ResetMissionControlRequest,
};

use super::{
    lnrpc::{
        htlc_attempt::HtlcStatus, lightning_client::LightningClient, payment::PaymentStatus, Hop,
        ListPaymentsRequest,
    },
    routerrpc::{router_client::RouterClient, SendPaymentRequest, TrackPaymentRequest},
    Repository, RepositoryError,
};

pub struct ClientConnection {
    pub address: Uri,
    pub ca_cert: Certificate,
    pub macaroon: String,
}

#[derive(Debug)]
pub struct Client<R>
where
    R: Repository,
{
    pub(super) network: Network,
    address: Uri,
    tls_config: ClientTlsConfig,
    macaroon: MetadataValue<Ascii>,
    repository: Arc<R>,
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

impl From<tonic::metadata::errors::InvalidMetadataValue> for GetClientError {
    fn from(value: tonic::metadata::errors::InvalidMetadataValue) -> Self {
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

impl<R> Client<R>
where
    R: Repository,
{
    pub fn new(
        connection: ClientConnection,
        network: Network,
        repository: Arc<R>,
    ) -> Result<Self, String> {
        let tls_config = ClientTlsConfig::new().ca_certificate(connection.ca_cert);
        Ok(Self {
            address: connection.address,
            network,
            tls_config,
            macaroon: connection
                .macaroon
                .parse()
                .map_err(|e: InvalidMetadataValue| e.to_string())?,
            repository,
        })
    }

    async fn get_channel(&self) -> Result<Channel, GetClientError> {
        let channel = match Channel::builder(self.address.clone())
            .tls_config(self.tls_config.clone())?
            .connect()
            .await
        {
            Ok(channel) => channel,
            Err(e) => {
                error!("failed to connect to lnd: {:?}", e);
                return Err(e.into());
            }
        };

        Ok(channel)
    }

    pub(super) async fn get_client(
        &self,
    ) -> Result<
        LightningClient<
            InterceptedService<Channel, impl Fn(Request<()>) -> Result<Request<()>, Status>>,
        >,
        GetClientError,
    > {
        let channel = self.get_channel().await?;
        let macaroon = self.macaroon.clone();
        let client = LightningClient::with_interceptor(channel, move |mut req: Request<()>| {
            req.metadata_mut().insert("macaroon", macaroon.clone());
            Ok(req)
        });

        Ok(client)
    }

    pub(super) async fn get_router_client(
        &self,
    ) -> Result<
        RouterClient<
            InterceptedService<Channel, impl Fn(Request<()>) -> Result<Request<()>, Status>>,
        >,
        GetClientError,
    > {
        let channel = self.get_channel().await?;
        let macaroon = self.macaroon.clone();
        let client = RouterClient::with_interceptor(channel, move |mut req: Request<()>| {
            req.metadata_mut().insert("macaroon", macaroon.clone());
            Ok(req)
        });

        Ok(client)
    }
}

#[async_trait::async_trait]
impl<R> crate::lightning::LightningClient for Client<R>
where
    R: Repository + Send + Sync,
{
    #[instrument(level = "trace", skip(self))]
    async fn get_payment_state(
        &self,
        hash: sha256::Hash,
        label: &str,
    ) -> Result<PaymentState, LightningError> {
        let payment_index = self.repository.get_payment_index(label).await?;
        let mut client = self.get_client().await?;
        let mut router_client = self.get_router_client().await?;
        let mut payment = None;
        if let Some(payment_index) = payment_index {
            let resp = client
                .list_payments(ListPaymentsRequest {
                    include_incomplete: true,
                    index_offset: payment_index,
                    max_payments: 1,
                    ..Default::default()
                })
                .await?
                .into_inner();
            payment = resp.payments.into_iter().next();
        }

        // TODO: The part below might fetch the latest version of a payment, rather than the one requested by the label. What to do?
        //       That might interfere with the active locks that shouldn't be removed.
        if payment.is_none() {
            let res = router_client
                .track_payment_v2(TrackPaymentRequest {
                    payment_hash: hash.as_byte_array().to_vec(),
                    no_inflight_updates: false,
                })
                .await;
            let mut stream = match res {
                Ok(res) => res.into_inner(),
                Err(e) => {
                    return match e.code() {
                        tonic::Code::NotFound => Err(LightningError::PaymentNotFound),
                        _ => Err(LightningError::General(e)),
                    }
                }
            };

            // TODO: Get or insert label.
            payment = Some(
                stream
                    .message()
                    .await?
                    .ok_or(LightningError::ConnectionFailed)?,
            );
        }

        let payment = payment.ok_or(LightningError::PaymentNotFound)?;
        match payment.status() {
            PaymentStatus::Unknown => Ok(PaymentState::Pending),
            PaymentStatus::InFlight => Ok(PaymentState::Pending),
            PaymentStatus::Succeeded => {
                let preimage = hex::decode(payment.payment_preimage)
                    .map_err(|_| LightningError::InvalidPreimage)?
                    .try_into()
                    .map_err(|_| LightningError::InvalidPreimage)?;
                Ok(PaymentState::Success { preimage })
            }
            PaymentStatus::Failed => Ok(PaymentState::Failure {
                error: String::from(payment.failure_reason().as_str_name()),
            }),
            PaymentStatus::Initiated => Ok(PaymentState::Pending),
        }
    }
    async fn get_preimage(
        &self,
        hash: sha256::Hash,
    ) -> Result<Option<PreimageResult>, LightningError> {
        let mut router_client = self.get_router_client().await?;
        let res = router_client
            .track_payment_v2(TrackPaymentRequest {
                payment_hash: hash.as_byte_array().to_vec(),
                no_inflight_updates: false,
            })
            .await;
        let mut stream = match res {
            Ok(res) => res.into_inner(),
            Err(e) => {
                return match e.code() {
                    tonic::Code::NotFound => Ok(None),
                    _ => Err(LightningError::General(e)),
                }
            }
        };
        let payment = match stream.message().await? {
            Some(message) => message,
            None => return Err(LightningError::ConnectionFailed),
        };

        if payment.payment_preimage.is_empty() {
            return Ok(None);
        }

        if payment.payment_preimage
            == "0000000000000000000000000000000000000000000000000000000000000000"
        {
            return Ok(None);
        }

        let preimage = hex::decode(&payment.payment_preimage)
            .map_err(|_| LightningError::InvalidPreimage)?
            .try_into()
            .map_err(|_| LightningError::InvalidPreimage)?;

        let label = self
            .repository
            .get_label(payment.payment_index)
            .await?
            .unwrap_or(String::from(""));
        Ok(Some(PreimageResult { preimage, label }))
    }

    #[instrument(level = "trace", skip(self))]
    async fn has_pending_or_complete_payment(
        &self,
        hash: &sha256::Hash,
    ) -> Result<bool, LightningError> {
        let mut router_client = self.get_router_client().await?;
        let res = router_client
            .track_payment_v2(TrackPaymentRequest {
                payment_hash: hash.as_byte_array().to_vec(),
                no_inflight_updates: false,
            })
            .await;
        let mut stream = match res {
            Ok(res) => res.into_inner(),
            Err(e) => {
                return match e.code() {
                    tonic::Code::NotFound => Ok(false),
                    _ => Err(LightningError::General(e)),
                }
            }
        };
        let payment = match stream.message().await? {
            Some(message) => message,
            None => return Err(LightningError::ConnectionFailed),
        };
        Ok(match payment.status() {
            PaymentStatus::Unknown => true,
            PaymentStatus::InFlight => true,
            PaymentStatus::Succeeded => true,
            PaymentStatus::Failed => false,
            PaymentStatus::Initiated => true,
        })
    }

    #[instrument(level = "trace", skip(self))]
    async fn pay(
        &self,
        request: crate::lightning::PaymentRequest,
    ) -> Result<PaymentResult, LightningError> {
        let mut router_client = self.get_router_client().await?;
        router_client
            .reset_mission_control(ResetMissionControlRequest::default())
            .await?;
        let mut stream = router_client
            .send_payment_v2(SendPaymentRequest {
                payment_request: request.bolt11,
                fee_limit_msat: request.fee_limit_msat as i64,
                timeout_seconds: request.timeout_seconds as i32,
                cltv_limit: request.cltv_limit as i32,
                ..Default::default()
            })
            .await
            .map_err(|e| {
                error!("send_payment_v2 returned error: {:?}", e);
                e
            })?
            .into_inner();
        let mut is_first_update = true;
        while let Some(update) = stream.message().await.map_err(|e| {
            error!("send_payment_v2 message stream returned error: {:?}", e);
            e
        })? {
            if is_first_update {
                is_first_update = false;
                self.repository
                    .add_label(request.label.clone(), update.payment_index)
                    .await?;
            }

            if let Some(last_attempt) = update.htlcs.last() {
                let route = last_attempt
                    .route
                    .as_ref()
                    .map(|route| hops_to_string(&route.hops));
                match last_attempt.status() {
                    HtlcStatus::InFlight => trace!(route, "sending htlc"),
                    HtlcStatus::Succeeded => trace!(route, "htlc succeeded"),
                    HtlcStatus::Failed => match &last_attempt.failure {
                        Some(failure) => {
                            let failure_source = match &last_attempt.route {
                                Some(route) => {
                                    if route.hops.len() > failure.failure_source_index as usize {
                                        let hop =
                                            &route.hops[failure.failure_source_index as usize];
                                        short_channel_id_to_string(hop.chan_id)
                                    } else {
                                        String::from("unknown")
                                    }
                                }
                                None => String::from("unknown"),
                            };
                            trace!(
                                code = field::display(failure.code().as_str_name()),
                                failure_source = field::display(failure_source),
                                route,
                                "htlc failed"
                            );
                        }
                        None => trace!(route, "htlc failed for unknown reason"),
                    },
                }
            }

            match update.status() {
                PaymentStatus::Unknown => trace!("payment status: unknown"),
                PaymentStatus::InFlight => trace!("payment status: in flight"),
                PaymentStatus::Succeeded => {
                    trace!("payment status: succeeded");
                    let preimage = hex::decode(update.payment_preimage)
                        .map_err(|_| LightningError::InvalidPreimage)?
                        .try_into()
                        .map_err(|_| LightningError::InvalidPreimage)?;
                    return Ok(PaymentResult::Success { preimage });
                }
                PaymentStatus::Failed => {
                    trace!(
                        reason = field::display(update.failure_reason().as_str_name()),
                        "payment status: failed"
                    );
                    return Ok(PaymentResult::Failure {
                        error: String::from(update.failure_reason().as_str_name()),
                    });
                }
                PaymentStatus::Initiated => trace!("payment status: initiated"),
            }
        }

        warn!("payment ended without final status");
        Err(LightningError::General(Status::internal(
            "did not receive final update from payment",
        )))
    }
}

fn hops_to_string(hops: &[Hop]) -> String {
    if hops.is_empty() {
        return String::from("");
    }

    let mut result = short_channel_id_to_string(hops[0].chan_id);
    for hop in hops.iter().skip(1) {
        result.push_str(" -> ");
        result.push_str(&short_channel_id_to_string(hop.chan_id));
    }

    result
}

fn short_channel_id_to_string(scid: u64) -> String {
    let block = (scid >> 40) & 0xffffffu64;
    let tx_index = (scid >> 16) & 0xffffffu64;
    let outnum = scid & 0xffffu64;
    format!("{}x{}x{}", block, tx_index, outnum)
}

impl From<RepositoryError> for LightningError {
    fn from(value: RepositoryError) -> Self {
        LightningError::General(Status::internal(value.to_string()))
    }
}
