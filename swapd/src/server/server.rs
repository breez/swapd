use bitcoin::{
    hashes::{sha256::Hash, Hash as _},
    Address, Network, PublicKey,
};
use lightning_invoice::Bolt11Invoice;
use std::fmt::Debug;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{debug, error, field, instrument, trace, warn};

use crate::{
    chain::{ChainClient, ChainError, FeeEstimateError, FeeEstimator, Utxo},
    chain_filter::ChainFilterService,
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

#[derive(Debug)]
pub struct SwapServer<C, CF, L, P, R, F>
where
    C: ChainClient,
    CF: ChainFilterService,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    F: FeeEstimator,
{
    network: Network,
    max_swap_amount_sat: u64,
    min_confirmations: u32,
    min_redeem_blocks: u32,
    chain_service: Arc<C>,
    chain_filter_service: Arc<CF>,
    lightning_client: Arc<L>,
    swap_service: Arc<SwapService<P>>,
    swap_repository: Arc<R>,
    fee_estimator: Arc<F>,
}

impl<C, CF, L, P, R, F> SwapServer<C, CF, L, P, R, F>
where
    C: ChainClient,
    CF: ChainFilterService,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    F: FeeEstimator,
{
    pub fn new(
        params: &SwapServerParams,
        chain_service: Arc<C>,
        chain_filter_service: Arc<CF>,
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
            chain_service,
            chain_filter_service,
            lightning_client,
            swap_service,
            swap_repository,
            fee_estimator,
        }
    }
}
#[tonic::async_trait]
impl<C, CF, L, P, R, F> Swapper for SwapServer<C, CF, L, P, R, F>
where
    C: ChainClient + Debug + Send + Sync + 'static,
    CF: ChainFilterService + Debug + Send + Sync + 'static,
    L: LightningClient + Debug + Send + Sync + 'static,
    P: PrivateKeyProvider + Debug + Send + Sync + 'static,
    R: SwapRepository + Debug + Send + Sync + 'static,
    F: FeeEstimator + Debug + Send + Sync + 'static,
{
    #[instrument(skip(self), level = "debug")]
    async fn add_fund_init(
        &self,
        request: Request<AddFundInitRequest>,
    ) -> Result<Response<AddFundInitReply>, Status> {
        debug!("add_fund_init request");
        let req = request.into_inner();
        // TODO: Return this error in error message?
        let payer_pubkey = PublicKey::from_slice(&req.pubkey).map_err(|_| {
            trace!("got invalid pubkey");
            Status::invalid_argument("invalid pubkey")
        })?;
        // TODO: Return this error in error message?
        let hash = Hash::from_slice(&req.hash).map_err(|_| {
            trace!("got invalid hash");
            Status::invalid_argument("invalid hash")
        })?;

        // Get a fee estimate for the next block to account for worst case fees.
        let fee_estimate = self.fee_estimator.estimate_fee(1).await?;

        // Assume a weight of 1000 for the transaction
        let min_allowed_deposit = fee_estimate.sat_per_kw.saturating_mul(3) / 2;

        let swap = self.swap_service.create_swap(payer_pubkey, hash)?;
        self.swap_repository.add_swap(&swap).await?;

        Ok(Response::new(AddFundInitReply {
            address: swap.public.address.to_string(),
            error_message: String::default(),
            lock_height: swap.public.lock_time as i64,
            max_allowed_deposit: self.max_swap_amount_sat as i64,
            min_allowed_deposit: min_allowed_deposit as i64,
            pubkey: swap.public.swapper_pubkey.to_bytes(),
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn add_fund_status(
        &self,
        request: Request<AddFundStatusRequest>,
    ) -> Result<Response<AddFundStatusReply>, Status> {
        debug!("add_fund_status request");
        let req = request.into_inner();
        let addresses = req
            .addresses
            .iter()
            .map(|a| {
                let a = match a.parse::<Address<_>>() {
                    Ok(a) => a,
                    Err(e) => {
                        trace!("got invalid address: {:?}", e);
                        return Err(Status::invalid_argument("invalid address"));
                    }
                };
                let a = match a.require_network(self.network) {
                    Ok(a) => a,
                    Err(_) => {
                        trace!("got invalid address (invalid network)");
                        return Err(Status::invalid_argument("invalid address"));
                    }
                };
                Ok(a)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let states = self
            .swap_repository
            .get_state(addresses)
            .await
            .map_err(|e| {
                error!("failed to get swap state: {:?}", e);
                Status::internal("internal error")
            })?;

        // TODO: addresses could have multiple utxos.
        Ok(Response::new(AddFundStatusReply {
            statuses: states
                .into_iter()
                .map(|s| {
                    let status = match s.status {
                        AddressStatus::Unknown => swap_api::AddressStatus::default(),
                        AddressStatus::Mempool { tx_info } => swap_api::AddressStatus {
                            amount: tx_info.amount as i64,
                            tx: tx_info.tx.to_string(),
                            ..Default::default()
                        },
                        AddressStatus::Confirmed {
                            block_hash,
                            block_height: _,
                            tx_info,
                        } => swap_api::AddressStatus {
                            amount: tx_info.amount as i64,
                            tx: tx_info.tx.to_string(),
                            block_hash: block_hash.to_string(),
                            confirmed: true,
                        },
                    };

                    (s.address.to_string(), status)
                })
                .collect(),
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn get_swap_payment(
        &self,
        request: Request<GetSwapPaymentRequest>,
    ) -> Result<Response<GetSwapPaymentReply>, Status> {
        debug!("get_swap_payment request");
        let req = request.into_inner();
        let invoice: Bolt11Invoice = req.payment_request.parse().map_err(|e| {
            trace!("got invalid payment request: {:?}", e);
            Status::invalid_argument("invalid payment request")
        })?;

        let amount_msat = match invoice.amount_milli_satoshis() {
            Some(amount_msat) => amount_msat,
            None => {
                trace!("got payment request without amount");
                return Err(Status::invalid_argument(
                    "payment request must have an amount",
                ));
            }
        };

        let amount_sat = amount_msat / 1000;
        if amount_sat * 1000 != amount_msat {
            trace!(amount_msat, "invoice amount is not a round sat amount");
            return Err(Status::invalid_argument(
                "invoice amount must be a round satoshi amount",
            ));
        }

        if amount_sat > self.max_swap_amount_sat {
            trace!(
                amount_sat,
                max_swap_amount_sat = self.max_swap_amount_sat,
                "invoice amount exceeds max swap amount"
            );
            return Err(Status::invalid_argument(
                "amount exceeds maximum allowed deposit",
            ));
        }

        let swap_state = self
            .swap_repository
            .get_swap_state_by_hash(invoice.payment_hash())
            .await?;
        if swap_state.utxos.is_empty() {
            trace!("swap has no utxos");
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
                    debug!(
                        outpoint = field::display(utxo.outpoint),
                        confirmations,
                        min_confirmations = self.min_confirmations,
                        "utxo has less than min confirmations"
                    );
                    return false;
                }

                if confirmations > max_confirmations {
                    debug!(
                        outpoint = field::display(utxo.outpoint),
                        confirmations, max_confirmations, "utxo has more than max confirmations"
                    );
                    return false;
                }

                trace!(
                    outpoint = field::display(utxo.outpoint),
                    confirmations,
                    min_confirmations = self.min_confirmations,
                    max_confirmations,
                    "utxo has correct amount of confirmations"
                );
                true
            })
            .collect::<Vec<Utxo>>();

        let utxos = match self.chain_filter_service.filter_utxos(&utxos).await {
            Ok(utxos) => utxos,
            Err(e) => {
                error!("failed to filter utxos: {:?}", e);
                utxos
            }
        };

        // TODO: Add ability to charge a fee?
        let amount_sum_sat = utxos.iter().fold(0u64, |sum, utxo| sum + utxo.amount_sat);
        if amount_sum_sat != amount_sat {
            trace!(
                amount_sum_sat,
                amount_sat,
                "utxo values don't match invoice value"
            );
            return Err(Status::failed_precondition(
                "confirmed utxo values don't match invoice value",
            ));
        }

        let fee_estimate = self.fee_estimator.estimate_fee(6).await?;
        let fake_address = Address::p2wpkh(
            &PublicKey::from_slice(&[0x04; 33]).map_err(|e| {
                error!("failed to create fake pubkey: {:?}", e);
                Status::internal("internal error")
            })?,
            self.network,
        )
        .map_err(|e| {
            error!("failed to create fake address: {:?}", e);
            Status::internal("internal error")
        })?;

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
            .map_err(|e| {
                debug!("could not create valid fake redeem tx: {:?}", e);
                Status::failed_precondition("value too low")
            })?;

        // TODO: Insert payment?

        let preimage = self.lightning_client.pay(req.payment_request).await?;
        match self
            .swap_repository
            .add_preimage(&swap_state.swap, &preimage)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!(
                    hash = field::display(swap_state.swap.public.hash),
                    preimage = hex::encode(preimage),
                    "failed to persist preimage: {:?}",
                    e
                );
            }
        };

        Ok(Response::new(GetSwapPaymentReply::default()))
    }
}

impl From<SwapPersistenceError> for Status {
    fn from(value: SwapPersistenceError) -> Self {
        match value {
            SwapPersistenceError::AlreadyExists => {
                trace!("swap already exists");
                Status::already_exists("Hash already exists")
            }
            SwapPersistenceError::General(e) => {
                error!("failed to persist swap: {:?}", e);
                Status::internal("internal error")
            }
        }
    }
}

impl From<GetSwapError> for Status {
    fn from(value: GetSwapError) -> Self {
        match value {
            GetSwapError::NotFound => {
                trace!("swap not found");
                Status::not_found("swap not found")
            }
            GetSwapError::General(e) => {
                error!("failed to get swap: {:?}", e);
                Status::internal("internal error")
            }
        }
    }
}

impl From<CreateSwapError> for Status {
    fn from(value: CreateSwapError) -> Self {
        match value {
            CreateSwapError::PrivateKeyError => {
                error!("failed to create swap due to private key error.");
                Status::internal("internal error")
            }
        }
    }
}

impl From<ChainError> for Status {
    fn from(value: ChainError) -> Self {
        match value {
            ChainError::General(e) => {
                error!("failed to access chain client: {:?}", e);
                Status::internal("internal error")
            }
        }
    }
}

impl From<FeeEstimateError> for Status {
    fn from(value: FeeEstimateError) -> Self {
        match value {
            FeeEstimateError::General(e) => {
                error!("failed to estimate fee: {:?}", e);
                Status::internal("internal error")
            }
            FeeEstimateError::Unavailable => {
                warn!("fee estimate is unavailable");
                Status::internal("internal error")
            }
        }
    }
}

impl From<PayError> for Status {
    fn from(value: PayError) -> Self {
        debug!("payment failed: {:?}", value);
        Status::unknown("payment failed")
    }
}
