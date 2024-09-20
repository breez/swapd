use std::sync::Arc;

use bitcoin::{
    address::{NetworkChecked, NetworkUnchecked},
    hashes::{sha256, Hash},
    Address, Network,
};
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use tracing::{instrument, warn};

use crate::{
    chain::ChainRepository,
    chain_filter::ChainFilterRepository,
    swap::{GetSwapError, SwapRepository},
};

use internal_swap_api::{
    swap_manager_server::SwapManager, AddAddressFiltersReply, AddAddressFiltersRequest,
    GetInfoReply, GetInfoRequest, GetSwapReply, GetSwapRequest, StopReply, StopRequest, SwapOutput,
};

pub mod internal_swap_api {
    tonic::include_proto!("swap_internal");
}

#[derive(Debug)]
pub struct Server<CF, CR, SR>
where
    CF: ChainFilterRepository,
    CR: ChainRepository,
    SR: SwapRepository,
{
    chain_filter_repository: Arc<CF>,
    chain_repository: Arc<CR>,
    network: Network,
    swap_repository: Arc<SR>,
    token: CancellationToken,
}

impl<CF, CR, SR> Server<CF, CR, SR>
where
    CR: ChainRepository,
    CF: ChainFilterRepository,
    SR: SwapRepository,
{
    pub fn new(
        network: Network,
        chain_filter_repository: Arc<CF>,
        chain_repository: Arc<CR>,
        swap_repository: Arc<SR>,
        token: CancellationToken,
    ) -> Self {
        Self {
            network,
            chain_filter_repository,
            chain_repository,
            swap_repository,
            token,
        }
    }
}

#[tonic::async_trait]
impl<CF, CR, SR> SwapManager for Server<CF, CR, SR>
where
    CF: ChainFilterRepository + Send + Sync + 'static,
    CR: ChainRepository + Send + Sync + 'static,
    SR: SwapRepository + Send + Sync + 'static,
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
                            GetSwapError::NotFound => Status::not_found("swap not found"),
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
                            GetSwapError::NotFound => Status::not_found("swap not found"),
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
                            GetSwapError::NotFound => Status::not_found("swap not found"),
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
    async fn stop(&self, _request: Request<StopRequest>) -> Result<Response<StopReply>, Status> {
        self.token.cancel();
        Ok(Response::new(StopReply {}))
    }
}
