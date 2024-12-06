use std::sync::Arc;

use bitcoin::{
    address::{NetworkChecked, NetworkUnchecked},
    hashes::{sha256, Hash},
    Address, Network, OutPoint,
};
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use tracing::{instrument, warn};

use crate::{
    chain::{ChainClient, ChainRepository, FeeEstimate, FeeEstimator},
    chain_filter::ChainFilterRepository,
    redeem::{RedeemError, RedeemRepository, RedeemService, RedeemServiceError},
    swap::{GetSwapsError, PrivateKeyProvider, SwapRepository},
    wallet::{Wallet, WalletError},
};

use internal_swap_api::{
    swap_manager_server::SwapManager, AddAddressFiltersReply, AddAddressFiltersRequest,
    GetInfoReply, GetInfoRequest, GetSwapReply, GetSwapRequest, ListRedeemableReply,
    ListRedeemableRequest, RedeemReply, RedeemRequest, RedeemableUtxo, StopReply, StopRequest,
    SwapOutput,
};

pub mod internal_swap_api {
    tonic::include_proto!("swap_internal");
}

#[derive(Debug)]
pub struct ServerParams<CC, CF, CR, F, P, RR, SR, W>
where
    CC: ChainClient,
    CF: ChainFilterRepository,
    CR: ChainRepository,
    F: FeeEstimator,
    P: PrivateKeyProvider,
    RR: RedeemRepository,
    SR: SwapRepository,
    W: Wallet,
{
    pub chain_client: Arc<CC>,
    pub chain_filter_repository: Arc<CF>,
    pub chain_repository: Arc<CR>,
    pub fee_estimator: Arc<F>,
    pub network: Network,
    pub redeem_service: Arc<RedeemService<CC, CR, RR, SR, P>>,
    pub swap_repository: Arc<SR>,
    pub token: CancellationToken,
    pub wallet: Arc<W>,
}

#[derive(Debug)]
pub struct Server<CC, CF, CR, F, P, RR, SR, W>
where
    CC: ChainClient,
    CF: ChainFilterRepository,
    CR: ChainRepository,
    F: FeeEstimator,
    P: PrivateKeyProvider,
    RR: RedeemRepository,
    SR: SwapRepository,
    W: Wallet,
{
    chain_client: Arc<CC>,
    chain_filter_repository: Arc<CF>,
    chain_repository: Arc<CR>,
    fee_estimator: Arc<F>,
    network: Network,
    redeem_service: Arc<RedeemService<CC, CR, RR, SR, P>>,
    swap_repository: Arc<SR>,
    token: CancellationToken,
    wallet: Arc<W>,
}

impl<CC, CF, CR, F, P, RR, SR, W> Server<CC, CF, CR, F, P, RR, SR, W>
where
    CC: ChainClient,
    CF: ChainFilterRepository,
    CR: ChainRepository,
    F: FeeEstimator,
    P: PrivateKeyProvider,
    RR: RedeemRepository,
    SR: SwapRepository,
    W: Wallet,
{
    pub fn new(params: ServerParams<CC, CF, CR, F, P, RR, SR, W>) -> Self {
        Self {
            chain_client: params.chain_client,
            chain_filter_repository: params.chain_filter_repository,
            chain_repository: params.chain_repository,
            fee_estimator: params.fee_estimator,
            network: params.network,
            redeem_service: params.redeem_service,
            swap_repository: params.swap_repository,
            token: params.token,
            wallet: params.wallet,
        }
    }
}

#[tonic::async_trait]
impl<CC, CF, CR, F, P, RR, SR, W> SwapManager for Server<CC, CF, CR, F, P, RR, SR, W>
where
    CC: ChainClient + Send + Sync + 'static,
    CF: ChainFilterRepository + Send + Sync + 'static,
    CR: ChainRepository + Send + Sync + 'static,
    F: FeeEstimator + Send + Sync + 'static,
    P: PrivateKeyProvider + Send + Sync + 'static,
    RR: RedeemRepository + Send + Sync + 'static,
    SR: SwapRepository + Send + Sync + 'static,
    W: Wallet + Send + Sync + 'static,
{
    #[instrument(skip(self), level = "debug")]
    async fn add_address_filters(
        &self,
        request: Request<AddAddressFiltersRequest>,
    ) -> Result<Response<AddAddressFiltersReply>, Status> {
        let req = request.into_inner();
        let addresses: Vec<Address<NetworkChecked>> = req
            .addresses
            .iter()
            .filter_map(|a| {
                let unchecked: Address<NetworkUnchecked> = match a.parse() {
                    Ok(a) => a,
                    Err(e) => {
                        warn!(
                            "Got invalid address '{}' in add_address_filters: {:?}",
                            a, e
                        );
                        return None;
                    }
                };

                let checked = match unchecked.require_network(self.network) {
                    Ok(a) => a,
                    Err(e) => {
                        warn!(
                            "Address '{}' in add_address_filters has invalid network: {:?}",
                            a, e
                        );
                        return None;
                    }
                };

                Some(checked)
            })
            .collect();

        self.chain_filter_repository
            .add_filter_addresses(&addresses)
            .await
            .map_err(|e| Status::internal(format!("failed to insert addresses: {:?}", e)))?;
        Ok(Response::new(AddAddressFiltersReply {}))
    }

    #[instrument(skip(self), level = "debug")]
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> Result<Response<GetInfoReply>, Status> {
        let tip = self.chain_repository.get_tip().await?;
        Ok(Response::new(GetInfoReply {
            block_height: tip.map(|tip| tip.height).unwrap_or(0u64),
            network: self.network.to_string(),
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn get_swap(
        &self,
        request: Request<GetSwapRequest>,
    ) -> Result<Response<GetSwapReply>, Status> {
        let request = request.into_inner();
        let swap = match (
            request.address,
            request.payment_request,
            request.payment_hash,
        ) {
            (Some(address), None, None) => {
                let address: Address<NetworkUnchecked> = address
                    .parse()
                    .map_err(|_| Status::invalid_argument("invalid address"))?;
                let address = address
                    .require_network(self.network)
                    .map_err(|_| Status::invalid_argument("invalid network for address"))?;
                match self.swap_repository.get_swap_by_address(&address).await {
                    Ok(swap) => swap,
                    Err(e) => {
                        return Err(match e {
                            GetSwapsError::NotFound => Status::not_found("swap not found"),
                            _ => Status::internal(format!("{:?}", e)),
                        })
                    }
                }
            }
            (None, Some(payment_request), None) => {
                match self
                    .swap_repository
                    .get_swap_by_payment_request(&payment_request)
                    .await
                {
                    Ok(swap) => swap,
                    Err(e) => {
                        return Err(match e {
                            GetSwapsError::NotFound => Status::not_found("swap not found"),
                            _ => Status::internal(format!("{:?}", e)),
                        })
                    }
                }
            }
            (None, None, Some(payment_hash)) => {
                let payment_hash = sha256::Hash::from_slice(&payment_hash)
                    .map_err(|_| Status::invalid_argument("invalid payment hash"))?;
                match self.swap_repository.get_swap_by_hash(&payment_hash).await {
                    Ok(swap) => swap,
                    Err(e) => {
                        return Err(match e {
                            GetSwapsError::NotFound => Status::not_found("swap not found"),
                            _ => Status::internal(format!("{:?}", e)),
                        })
                    }
                }
            }
            _ => {
                return Err(Status::invalid_argument(
                    "one of the parameters must be set",
                ))
            }
        };

        let utxos = self
            .chain_repository
            .get_utxos_for_address(&swap.swap.public.address)
            .await
            .map_err(|e| Status::internal(format!("{:?}", e)))?;
        let reply = GetSwapReply {
            address: swap.swap.public.address.to_string(),
            outputs: utxos
                .iter()
                .map(|utxo| SwapOutput {
                    confirmation_height: Some(utxo.block_height),
                    outpoint: utxo.outpoint.to_string(),
                })
                .collect(),
        };
        Ok(Response::new(reply))
    }

    #[instrument(skip(self), level = "debug")]
    async fn list_redeemable(
        &self,
        _request: Request<ListRedeemableRequest>,
    ) -> Result<Response<ListRedeemableReply>, Status> {
        let current_height = self.chain_client.get_blockheight().await?;
        let redeemables = self.redeem_service.list_redeemable().await?;
        Ok(Response::new(ListRedeemableReply {
            redeemables: redeemables
                .into_iter()
                .map(|r| RedeemableUtxo {
                    outpoint: r.utxo.outpoint.to_string(),
                    swap_hash: r.swap.public.hash.to_string(),
                    lock_time: r.swap.public.lock_time,
                    confirmation_height: r.utxo.block_height,
                    blocks_left: r.blocks_left(current_height),
                    paid_with_request: r.paid_with_request,
                })
                .collect(),
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn redeem(
        &self,
        request: Request<RedeemRequest>,
    ) -> Result<Response<RedeemReply>, Status> {
        let request = request.into_inner();
        let all_redeemables = self.redeem_service.list_redeemable().await?;
        let mut redeemables = Vec::new();
        for outpoint in request.outpoints {
            let outpoint: OutPoint = outpoint
                .parse()
                .map_err(|_| Status::invalid_argument(format!("invalid outpoint {}", outpoint)))?;
            let redeemable = match all_redeemables.iter().find(|r| r.utxo.outpoint == outpoint) {
                Some(redeemable) => redeemable,
                None => {
                    return Err(Status::invalid_argument(format!(
                        "outpoint {} not found",
                        outpoint
                    )))
                }
            };
            redeemables.push(redeemable.clone());
        }

        let current_height = self.chain_client.get_blockheight().await?;
        let min_blocks_left = match redeemables
            .iter()
            .map(|r| r.blocks_left(current_height))
            .min()
        {
            Some(m) => m,
            None => return Err(Status::invalid_argument("no outpoints selected")),
        };

        let fee_estimate = match request.fee_per_kw {
            Some(fee_per_kw) => FeeEstimate {
                sat_per_kw: fee_per_kw,
            },
            None => self.fee_estimator.estimate_fee(min_blocks_left).await?,
        };

        let destination_address = match request.destination_address {
            Some(a) => a
                .parse::<Address<NetworkUnchecked>>()
                .map_err(|e| Status::invalid_argument(e.to_string()))?
                .require_network(self.network)
                .map_err(|e| Status::invalid_argument(e.to_string()))?,
            None => self.wallet.new_address().await?,
        };

        let tx = self
            .redeem_service
            .redeem(
                &redeemables,
                &fee_estimate,
                current_height,
                destination_address,
                request.auto_bump,
            )
            .await?;
        Ok(Response::new(RedeemReply {
            tx_id: tx.compute_txid().to_string(),
            fee_per_kw: fee_estimate.sat_per_kw,
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn stop(&self, _request: Request<StopRequest>) -> Result<Response<StopReply>, Status> {
        self.token.cancel();
        Ok(Response::new(StopReply {}))
    }
}

impl From<RedeemServiceError> for Status {
    fn from(value: RedeemServiceError) -> Self {
        Status::internal(value.to_string())
    }
}

impl From<WalletError> for Status {
    fn from(value: WalletError) -> Self {
        Status::internal(value.to_string())
    }
}

impl From<RedeemError> for Status {
    fn from(value: RedeemError) -> Self {
        Status::internal(value.to_string())
    }
}
