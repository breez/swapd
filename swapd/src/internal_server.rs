use std::sync::Arc;

use bitcoin::{
    address::{NetworkChecked, NetworkUnchecked},
    Address, Network,
};
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use tracing::{instrument, warn};

use crate::chain_filter::ChainFilterRepository;

use internal_swap_api::{
    swap_manager_server::SwapManager, AddAddressFiltersReply, AddAddressFiltersRequest,
    GetInfoReply, GetInfoRequest, GetSwapReply, GetSwapRequest, StopReply, StopRequest,
};

pub mod internal_swap_api {
    tonic::include_proto!("swap_internal");
}

#[derive(Debug)]
pub struct Server<R>
where
    R: ChainFilterRepository,
{
    chain_filter_repository: Arc<R>,
    network: Network,
    token: CancellationToken,
}

impl<R> Server<R>
where
    R: ChainFilterRepository,
{
    pub fn new(
        network: Network,
        chain_filter_repository: Arc<R>,
        token: CancellationToken,
    ) -> Self {
        Self {
            network,
            chain_filter_repository,
            token,
        }
    }
}

#[tonic::async_trait]
impl<R> SwapManager for Server<R>
where
    R: ChainFilterRepository + Send + Sync + 'static,
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
        todo!()
    }

    #[instrument(skip(self), level = "debug")]
    async fn get_swap(
        &self,
        _request: Request<GetSwapRequest>,
    ) -> Result<Response<GetSwapReply>, Status> {
        todo!()
    }

    #[instrument(skip(self), level = "debug")]
    async fn stop(&self, _request: Request<StopRequest>) -> Result<Response<StopReply>, Status> {
        self.token.cancel();
        Ok(Response::new(StopReply {}))
    }
}
