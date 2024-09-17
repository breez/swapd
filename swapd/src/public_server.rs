use bitcoin::{
    hashes::{sha256::Hash, Hash as _},
    Address, Network, PublicKey,
};
use lightning_invoice::Bolt11Invoice;
use std::sync::Arc;
use std::{
    fmt::Debug,
    time::{SystemTime, UNIX_EPOCH},
};
use tonic::{Request, Response, Status};
use tracing::{debug, error, field, instrument, trace, warn};

use crate::{
    chain::{
        ChainClient, ChainError, ChainRepository, ChainRepositoryError, FeeEstimateError,
        FeeEstimator, Utxo,
    },
    chain_filter::ChainFilterService,
    lightning::{LightningClient, PayError, PaymentRequest},
    public_server::swap_api::AddressStatus,
    swap::PaymentAttempt,
};

use crate::swap::{
    CreateSwapError, GetSwapError, PrivateKeyProvider, SwapPersistenceError, SwapRepository,
    SwapService,
};
use swap_api::{
    swapper_server::Swapper, AddFundInitReply, AddFundInitRequest, AddFundStatusReply,
    AddFundStatusRequest, GetSwapPaymentReply, GetSwapPaymentRequest,
};

pub mod swap_api {
    tonic::include_proto!("swap");
}

const FAKE_PREIMAGE: [u8; 32] = [0; 32];
pub struct SwapServerParams {
    pub network: Network,
    pub max_swap_amount_sat: u64,
    pub min_confirmations: u64,
    pub min_redeem_blocks: u32,
}

#[derive(Debug)]
pub struct SwapServer<C, CF, CR, L, P, R, F>
where
    C: ChainClient,
    CF: ChainFilterService,
    CR: ChainRepository,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    F: FeeEstimator,
{
    network: Network,
    max_swap_amount_sat: u64,
    min_confirmations: u64,
    min_redeem_blocks: u32,
    chain_service: Arc<C>,
    chain_filter_service: Arc<CF>,
    chain_repository: Arc<CR>,
    lightning_client: Arc<L>,
    swap_service: Arc<SwapService<P>>,
    swap_repository: Arc<R>,
    fee_estimator: Arc<F>,
}

impl<C, CF, CR, L, P, R, F> SwapServer<C, CF, CR, L, P, R, F>
where
    C: ChainClient,
    CF: ChainFilterService,
    CR: ChainRepository,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    F: FeeEstimator,
{
    pub fn new(
        params: &SwapServerParams,
        chain_service: Arc<C>,
        chain_filter_service: Arc<CF>,
        chain_repository: Arc<CR>,
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
            chain_repository,
            lightning_client,
            swap_service,
            swap_repository,
            fee_estimator,
        }
    }
}
#[tonic::async_trait]
impl<C, CF, CR, L, P, R, F> Swapper for SwapServer<C, CF, CR, L, P, R, F>
where
    C: ChainClient + Debug + Send + Sync + 'static,
    CF: ChainFilterService + Debug + Send + Sync + 'static,
    CR: ChainRepository + Debug + Send + Sync + 'static,
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
        self.chain_repository
            .add_watch_address(&swap.public.address)
            .await?;
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

        let swaps = self
            .swap_repository
            .get_swaps(&addresses)
            .await
            .map_err(|e| {
                error!("failed to get swap state: {:?}", e);
                Status::internal("internal error")
            })?;
        let address_utxos = self
            .chain_repository
            .get_utxos_for_addresses(&addresses)
            .await?;

        // TODO: addresses could have multiple utxos.
        // TODO: 'confirmed' doesn't make sense below, because they're always confirmed.
        Ok(Response::new(AddFundStatusReply {
            statuses: addresses
                .iter()
                .filter_map(|address| {
                    let _swap = match swaps.get(address) {
                        Some(swap) => swap,
                        None => return None,
                    };
                    let utxo = match address_utxos.get(address) {
                        Some(utxos) => match utxos.first() {
                            Some(utxo) => utxo,
                            None => return None,
                        },
                        None => return None,
                    };
                    Some((
                        address.to_string(),
                        AddressStatus {
                            amount: utxo.amount_sat as i64,
                            block_hash: utxo.block_hash.to_string(),
                            tx: utxo.outpoint.txid.to_string(),
                            confirmed: true,
                        },
                    ))
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

        let hash = invoice.payment_hash();
        let swap_state = self.swap_repository.get_swap(hash).await?;
        if swap_state.preimage.is_some() {
            trace!("swap already had preimage");
            return Err(Status::failed_precondition("swap already paid"));
        }

        let utxos = self
            .chain_repository
            .get_utxos_for_address(&swap_state.swap.public.address)
            .await?;

        if utxos.is_empty() {
            trace!("swap has no utxos");
            return Err(Status::failed_precondition("no utxos found"));
        }

        let current_height = self.chain_service.get_blockheight().await?;
        let max_confirmations = swap_state
            .swap
            .public
            .lock_time
            .saturating_sub(self.min_redeem_blocks) as u64;
        let utxos = utxos
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

        // TODO: Filter utxos on sync?
        let utxos = match self.chain_filter_service.filter_utxos(&utxos).await {
            Ok(utxos) => utxos,
            Err(e) => {
                error!("failed to filter utxos: {:?}", e);
                utxos
            }
        };

        // TODO: Add ability to charge a fee?
        // Sum the utxo amounts.
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

        // Do a fee estimation with 6 blocks in order to check whether the swap
        // is redeemable within reasonable time.
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

        // Store the payment attempt to ensure not 'too many' utxos are claimed
        // on redeem if a user accidentally sends multiple utxos to the same
        // address.
        let now = SystemTime::now();
        let unix_ns_now = now
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                error!("failed to get duration since unix epoch");
                Status::internal("internal error")
            })?
            .as_nanos();
        let label = format!("{}-{}", hash, unix_ns_now);
        self.swap_repository
            .add_payment_attempt(&PaymentAttempt {
                amount_msat,
                creation_time: now,
                label: label.clone(),
                destination: invoice.get_payee_pub_key(),
                payment_request: req.payment_request.clone(),
                payment_hash: swap_state.swap.public.hash,
                utxos,
            })
            .await?;

        // Pay the user. After the payment succeeds, we will have paid the
        // funds, but not redeemed anything onchain yet. That will happen in the
        // redeem module.
        // TODO: Add a maximum fee here?
        let pay_result = self
            .lightning_client
            .pay(PaymentRequest {
                bolt11: req.payment_request,
                payment_hash: *hash,
                label: label.clone(),
            })
            .await?;

        // Persist the preimage right away. There's also a background service
        // checking for preimages, in case the `pay` call failed, but the
        // payment did succeed.
        match self
            .swap_repository
            .add_payment_result(hash, &label, &pay_result)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!(
                    hash = field::display(swap_state.swap.public.hash),
                    result = field::debug(pay_result),
                    "failed to persist pay result: {:?}",
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
            GetSwapError::InvalidPreimage => {
                error!("got invalid preimage");
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
            ChainError::Database(e) => {
                error!("database error: {:?}", e);
                Status::internal("internal error")
            }
            ChainError::EmptyChain => {
                error!("got empty chain error");
                Status::internal("internal error")
            }
            ChainError::InvalidChain => {
                error!("got invalid chain error");
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

impl From<ChainRepositoryError> for Status {
    fn from(value: ChainRepositoryError) -> Self {
        match value {
            ChainRepositoryError::General(e) => {
                error!("failed to get chain data: {:?}", e);
                Status::internal("internal error")
            }
        }
    }
}
