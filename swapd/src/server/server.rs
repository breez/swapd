use std::sync::Arc;

use bitcoin::{
    hashes::{sha256::Hash, Hash as _},
    Address, Network, PublicKey,
};
use lightning_invoice::Bolt11Invoice;
use tonic::{Request, Response, Status};

use crate::{
    chain::{BlockListService, ChainClient, ChainError, FeeEstimateError, FeeEstimator, Utxo},
    lightning::{LightningClient, PayError},
};

use super::{
    privkey_provider::PrivateKeyProvider,
    swap_api::{
        self, swapper_server::Swapper, AddFundInitReply, AddFundInitRequest, AddFundStatusReply,
        AddFundStatusRequest, GetSwapPaymentReply, GetSwapPaymentRequest,
    },
    swap_repository::{AddressStatus, GetSwapError, SwapPersistenceError, SwapRepository},
    swap_service::CreateSwapError,
    SwapService,
};

const FAKE_PREIMAGE: [u8; 32] = [0; 32];
pub struct SwapServerParams {
    pub network: Network,
    pub max_swap_amount_sat: u64,
    pub min_confirmations: u32,
    pub min_redeem_blocks: u32,
}

pub struct SwapServer<B, C, L, P, R, F>
where
    B: BlockListService,
    C: ChainClient,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    F: FeeEstimator,
{
    network: Network,
    max_swap_amount_sat: u64,
    min_confirmations: u32,
    min_redeem_blocks: u32,
    block_list_service: Arc<B>,
    chain_service: Arc<C>,
    lightning_client: Arc<L>,
    swap_service: Arc<SwapService<P>>,
    swap_repository: Arc<R>,
    fee_estimator: Arc<F>,
}

impl<B, C, L, P, R, F> SwapServer<B, C, L, P, R, F>
where
    B: BlockListService,
    C: ChainClient,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    F: FeeEstimator,
{
    pub fn new(
        params: &SwapServerParams,
        block_list_service: Arc<B>,
        chain_service: Arc<C>,
        lightning_client: Arc<L>,
        swap_service: Arc<SwapService<P>>,
        swap_repository: Arc<R>,
        fee_estimator: Arc<F>,
    ) -> Self {
        SwapServer {
            network: params.network,
            min_confirmations: params.min_confirmations,
            min_redeem_blocks: params.min_redeem_blocks,
            max_swap_amount_sat: params.max_swap_amount_sat,
            block_list_service,
            chain_service,
            lightning_client,
            swap_service,
            swap_repository,
            fee_estimator,
        }
    }
}
#[tonic::async_trait]
impl<B, C, L, P, R, F> Swapper for SwapServer<B, C, L, P, R, F>
where
    B: BlockListService + Send + Sync + 'static,
    C: ChainClient + Send + Sync + 'static,
    L: LightningClient + Send + Sync + 'static,
    P: PrivateKeyProvider + Send + Sync + 'static,
    R: SwapRepository + Send + Sync + 'static,
    F: FeeEstimator + Send + Sync + 'static,
{
    async fn add_fund_init(
        &self,
        request: Request<AddFundInitRequest>,
    ) -> Result<Response<AddFundInitReply>, Status> {
        let req = request.into_inner();
        // TODO: Return this error in error message?
        let payer_pubkey = PublicKey::from_slice(&req.pubkey)
            .map_err(|_| Status::invalid_argument("invalid pubkey"))?;
        // TODO: Return this error in error message?
        let hash =
            Hash::from_slice(&req.hash).map_err(|_| Status::invalid_argument("invalid hash"))?;

        // Get a fee estimate for the next block to account for worst case fees.
        let fee_estimate = self
            .fee_estimator
            .estimate_fee(1)
            .await
            .map_err(|_| Status::internal("internal error"))?;

        // Assume a weight of 1000 for the transaction
        let min_allowed_deposit = fee_estimate.sat_per_kw.saturating_mul(3) / 2;

        let swap = self.swap_service.create_swap(payer_pubkey, hash)?;
        self.swap_repository.add_swap(&swap).await?;

        Ok(Response::new(AddFundInitReply {
            address: swap.public.address.to_string(),
            error_message: String::from(""),
            lock_height: swap.public.lock_time as i64,
            max_allowed_deposit: self.max_swap_amount_sat as i64,
            min_allowed_deposit: min_allowed_deposit as i64,
            pubkey: swap.public.swapper_pubkey.to_bytes(),
        }))
    }

    async fn add_fund_status(
        &self,
        request: Request<AddFundStatusRequest>,
    ) -> Result<Response<AddFundStatusReply>, Status> {
        let req = request.into_inner();
        let addresses = req
            .addresses
            .iter()
            .map(|a| {
                let a = match a.parse::<Address<_>>() {
                    Ok(a) => a,
                    Err(_) => return Err(Status::invalid_argument("invalid address")),
                };
                let a = match a.require_network(self.network) {
                    Ok(a) => a,
                    Err(_) => return Err(Status::invalid_argument("invalid address")),
                };
                Ok(a)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let states = self
            .swap_repository
            .get_state(addresses)
            .await
            .map_err(|_| Status::internal("internal error"))?;
        Ok(Response::new(AddFundStatusReply {
            statuses: states
                .into_iter()
                .map(|s| {
                    let status = match s.status {
                        AddressStatus::Unknown => swap_api::AddressStatus::default(),
                        AddressStatus::Mempool { tx_info } => swap_api::AddressStatus {
                            amount: tx_info.amount as i64,
                            tx: hex::encode(&tx_info.tx),
                            ..Default::default()
                        },
                        AddressStatus::Confirmed {
                            block_hash,
                            block_height: _,
                            tx_info,
                        } => swap_api::AddressStatus {
                            amount: tx_info.amount as i64,
                            tx: hex::encode(&tx_info.tx),
                            block_hash: block_hash.to_string(),
                            confirmed: true,
                        },
                    };

                    (s.address.to_string(), status)
                })
                .collect(),
        }))
    }

    async fn get_swap_payment(
        &self,
        request: Request<GetSwapPaymentRequest>,
    ) -> Result<Response<GetSwapPaymentReply>, Status> {
        let req = request.into_inner();
        let invoice: Bolt11Invoice = req
            .payment_request
            .parse()
            .map_err(|_| Status::invalid_argument("invalid payment request"))?;

        let amount_msat = match invoice.amount_milli_satoshis() {
            Some(amount_msat) => amount_msat,
            None => return Err(Status::invalid_argument("invoice must have an amount")),
        };

        let amount_sat = amount_msat / 1000;
        if amount_sat * 1000 != amount_msat {
            return Err(Status::invalid_argument(
                "invoice amount must be a round satoshi amount",
            ));
        }

        if amount_sat > self.max_swap_amount_sat {
            return Err(Status::invalid_argument(
                "amount exceeds maximum allowed deposit",
            ));
        }

        let swap_state = self
            .swap_repository
            .get_swap_state_by_hash(invoice.payment_hash())
            .await?;
        if swap_state.utxos.is_empty() {
            return Err(Status::failed_precondition("no utxos found"));
        }

        let current_height = self.chain_service.get_blockheight().await?;
        let max_confirmations = swap_state
            .swap
            .public
            .lock_time
            .saturating_sub(self.min_redeem_blocks);
        let utxos = swap_state
            .utxos
            .into_iter()
            .filter(|utxo| {
                let confirmations = current_height.saturating_sub(utxo.block_height);
                if confirmations < self.min_confirmations {
                    return false;
                }

                if confirmations > max_confirmations {
                    return false;
                }

                true
            })
            .collect::<Vec<Utxo>>();

        let utxos = match self.block_list_service.filter_blocklisted(&utxos).await {
            Ok(utxos) => utxos,
            Err(_) => utxos,
        };

        // TODO: Add ability to charge a fee?
        let amount_sum_sat = utxos.iter().fold(0u64, |sum, utxo| sum + utxo.amount_sat);
        if amount_sum_sat != amount_sat {
            return Err(Status::failed_precondition(
                "confirmed utxo values don't match invoice value",
            ));
        }

        let fee_estimate = self.fee_estimator.estimate_fee(6).await?;
        let fake_address = Address::p2wpkh(
            &PublicKey::from_slice(&[0x04; 33]).map_err(|_| Status::internal("internal error"))?,
            self.network,
        )
        .map_err(|_| Status::internal("internal error"))?;

        // If the redeem tx can be created, this is a valid swap.
        self.swap_service
            .create_redeem_tx(
                &swap_state.swap,
                &utxos,
                &fee_estimate,
                current_height,
                &FAKE_PREIMAGE,
                fake_address,
            )
            .map_err(|_| Status::failed_precondition("value too low"))?;

        // TODO: Insert payment?

        let preimage = self.lightning_client.pay(req.payment_request).await?;
        match self
            .swap_repository
            .add_preimage(&swap_state.swap, &preimage)
            .await
        {
            Ok(_) => {}
            Err(_) => todo!("log error"),
        };

        Ok(Response::new(GetSwapPaymentReply::default()))
    }
}

impl From<SwapPersistenceError> for Status {
    fn from(value: SwapPersistenceError) -> Self {
        match value {
            SwapPersistenceError::AlreadyExists => Status::already_exists("Hash already exists"),
            SwapPersistenceError::General(_) => Status::internal("internal error"),
        }
    }
}

impl From<GetSwapError> for Status {
    fn from(value: GetSwapError) -> Self {
        match value {
            GetSwapError::NotFound => Status::not_found("swap not found"),
            GetSwapError::General(_) => Status::internal("internal error"),
        }
    }
}

impl From<CreateSwapError> for Status {
    fn from(value: CreateSwapError) -> Self {
        match value {
            CreateSwapError::PrivateKeyError => Status::internal("internal error"),
        }
    }
}

impl From<ChainError> for Status {
    fn from(value: ChainError) -> Self {
        match value {
            ChainError::General(_) => Status::internal("internal error"),
        }
    }
}

impl From<FeeEstimateError> for Status {
    fn from(value: FeeEstimateError) -> Self {
        match value {
            FeeEstimateError::General(_) => Status::internal("internal error"),
            FeeEstimateError::Unavailable => Status::internal("internal error"),
        }
    }
}

impl From<PayError> for Status {
    fn from(_value: PayError) -> Self {
        Status::unknown("payment failed")
    }
}
