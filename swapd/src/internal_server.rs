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
    claim::{ClaimError, ClaimRepository, ClaimService, ClaimServiceError},
    swap::{GetSwapsError, PrivateKeyProvider, SwapRepository},
    wallet::{Wallet, WalletError},
};

use internal_swap_api::{
    swap_manager_server::SwapManager, AddAddressFiltersRequest, AddAddressFiltersResponse,
    ClaimRequest, ClaimResponse, ClaimableUtxo, GetInfoRequest, GetInfoResponse, GetSwapRequest,
    GetSwapResponse, ListClaimableRequest, ListClaimableResponse, StopRequest, StopResponse,
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
    RR: ClaimRepository,
    SR: SwapRepository,
    W: Wallet,
{
    pub chain_client: Arc<CC>,
    pub chain_filter_repository: Arc<CF>,
    pub chain_repository: Arc<CR>,
    pub fee_estimator: Arc<F>,
    pub network: Network,
    pub claim_service: Arc<ClaimService<CC, CR, RR, SR, P>>,
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
    RR: ClaimRepository,
    SR: SwapRepository,
    W: Wallet,
{
    chain_client: Arc<CC>,
    chain_filter_repository: Arc<CF>,
    chain_repository: Arc<CR>,
    fee_estimator: Arc<F>,
    network: Network,
    claim_service: Arc<ClaimService<CC, CR, RR, SR, P>>,
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
    RR: ClaimRepository,
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
            claim_service: params.claim_service,
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
    RR: ClaimRepository + Send + Sync + 'static,
    SR: SwapRepository + Send + Sync + 'static,
    W: Wallet + Send + Sync + 'static,
{
    #[instrument(skip(self), level = "debug")]
    async fn add_address_filters(
        &self,
        request: Request<AddAddressFiltersRequest>,
    ) -> Result<Response<AddAddressFiltersResponse>, Status> {
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
        Ok(Response::new(AddAddressFiltersResponse {}))
    }

    #[instrument(skip(self), level = "debug")]
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> Result<Response<GetInfoResponse>, Status> {
        let tip = self.chain_repository.get_tip().await?;
        Ok(Response::new(GetInfoResponse {
            block_height: tip.map(|tip| tip.height).unwrap_or(0u64),
            network: self.network.to_string(),
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn get_swap(
        &self,
        request: Request<GetSwapRequest>,
    ) -> Result<Response<GetSwapResponse>, Status> {
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
        let reply = GetSwapResponse {
            address: swap.swap.public.address.to_string(),
            outputs: utxos
                .iter()
                .map(|utxo| SwapOutput {
                    confirmation_height: Some(utxo.block_height),
                    outpoint: utxo.outpoint.to_string(),
                    block_hash: Some(utxo.block_hash.to_string()),
                })
                .collect(),
        };
        Ok(Response::new(reply))
    }

    #[instrument(skip(self), level = "debug")]
    async fn list_claimable(
        &self,
        _request: Request<ListClaimableRequest>,
    ) -> Result<Response<ListClaimableResponse>, Status> {
        let current_height = self.chain_client.get_blockheight().await?;
        let claimables = self.claim_service.list_claimable().await?;
        Ok(Response::new(ListClaimableResponse {
            claimables: claimables
                .into_iter()
                .map(|c| ClaimableUtxo {
                    outpoint: c.utxo.outpoint.to_string(),
                    swap_hash: c.swap.public.hash.to_string(),
                    lock_time: c.swap.public.lock_time.into(),
                    confirmation_height: c.utxo.block_height,
                    block_hash: c.utxo.block_hash.to_string(),
                    blocks_left: c.blocks_left(current_height),
                    paid_with_request: c.paid_with_request,
                })
                .collect(),
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn claim(
        &self,
        request: Request<ClaimRequest>,
    ) -> Result<Response<ClaimResponse>, Status> {
        let request = request.into_inner();
        let all_claimables = self.claim_service.list_claimable().await?;
        let mut claimables = Vec::new();
        for outpoint in request.outpoints {
            let outpoint: OutPoint = outpoint
                .parse()
                .map_err(|_| Status::invalid_argument(format!("invalid outpoint {}", outpoint)))?;
            let claimable = match all_claimables.iter().find(|c| c.utxo.outpoint == outpoint) {
                Some(claimable) => claimable,
                None => {
                    return Err(Status::invalid_argument(format!(
                        "outpoint {} not found",
                        outpoint
                    )))
                }
            };
            claimables.push(claimable.clone());
        }

        let current_height = self.chain_client.get_blockheight().await?;
        let min_blocks_left = match claimables
            .iter()
            .map(|c| c.blocks_left(current_height))
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
            .claim_service
            .claim(
                &claimables,
                &fee_estimate,
                current_height,
                destination_address,
                request.auto_bump,
            )
            .await?;
        Ok(Response::new(ClaimResponse {
            tx_id: tx.compute_txid().to_string(),
            fee_per_kw: fee_estimate.sat_per_kw,
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn stop(&self, _request: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        self.token.cancel();
        Ok(Response::new(StopResponse {}))
    }
}

impl From<ClaimServiceError> for Status {
    fn from(value: ClaimServiceError) -> Self {
        Status::internal(value.to_string())
    }
}

impl From<WalletError> for Status {
    fn from(value: WalletError) -> Self {
        Status::internal(value.to_string())
    }
}

impl From<ClaimError> for Status {
    fn from(value: ClaimError) -> Self {
        Status::internal(value.to_string())
    }
}
