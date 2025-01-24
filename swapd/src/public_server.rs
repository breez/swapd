use bitcoin::{
    address::NetworkUnchecked,
    consensus::Decodable,
    hashes::{sha256::Hash, Hash as _},
    secp256k1::PublicKey,
    Address, CompressedPublicKey, Network, Transaction, TxOut,
};
use futures::future::join_all;
use lightning_invoice::Bolt11Invoice;
use secp256k1::musig::MusigPubNonce;
use std::sync::Arc;
use std::{
    fmt::Debug,
    time::{SystemTime, UNIX_EPOCH},
};
use tonic::{Request, Response, Status};
use tracing::{debug, error, field, info, instrument, trace, warn};

use crate::{
    chain::{
        ChainClient, ChainError, ChainRepository, ChainRepositoryError, FeeEstimateError,
        FeeEstimator, Txo,
    },
    chain_filter::ChainFilterService,
    lightning::{LightningClient, LightningError, PaymentRequest, PaymentResult},
    swap::{ClaimableUtxo, LockSwapError, PaymentAttempt, RandomError, RandomProvider},
};

use crate::swap::{
    GetSwapsError, PrivateKeyProvider, SwapPersistenceError, SwapRepository, SwapService,
};
use swap_api::{
    swapper_server::Swapper, CreateSwapRequest, CreateSwapResponse, PaySwapRequest,
    PaySwapResponse, RefundSwapRequest, RefundSwapResponse, SwapParameters, SwapParametersRequest,
    SwapParametersResponse,
};

pub mod swap_api {
    tonic::include_proto!("swap");
}

const FAKE_PREIMAGE: [u8; 32] = [0; 32];
const MIN_SWAP_AMOUNT_CONF_TARGET: i32 = 12;
pub struct SwapServerParams<C, CF, CR, L, P, R, RP, F>
where
    C: ChainClient,
    CF: ChainFilterService,
    CR: ChainRepository,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    RP: RandomProvider,
    F: FeeEstimator,
{
    pub network: Network,
    pub max_swap_amount_sat: u64,
    pub min_confirmations: u64,
    pub min_claim_blocks: u32,
    pub min_viable_cltv: u32,
    pub pay_fee_limit_base_msat: u64,
    pub pay_fee_limit_ppm: u64,
    pub pay_timeout_seconds: u16,
    pub chain_service: Arc<C>,
    pub chain_filter_service: Arc<CF>,
    pub chain_repository: Arc<CR>,
    pub lightning_client: Arc<L>,
    pub random_provider: Arc<RP>,
    pub swap_service: Arc<SwapService<P>>,
    pub swap_repository: Arc<R>,
    pub fee_estimator: Arc<F>,
}

#[derive(Debug)]
pub struct SwapServer<C, CF, CR, L, P, R, RP, F>
where
    C: ChainClient,
    CF: ChainFilterService,
    CR: ChainRepository,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    RP: RandomProvider,
    F: FeeEstimator,
{
    network: Network,
    max_swap_amount_sat: u64,
    min_confirmations: u64,
    min_claim_blocks: u32,
    min_viable_cltv: u32,
    pay_fee_limit_base_msat: u64,
    pay_fee_limit_ppm: u64,
    pay_timeout_seconds: u16,
    chain_client: Arc<C>,
    chain_filter_service: Arc<CF>,
    chain_repository: Arc<CR>,
    lightning_client: Arc<L>,
    random_provider: Arc<RP>,
    swap_service: Arc<SwapService<P>>,
    swap_repository: Arc<R>,
    fee_estimator: Arc<F>,
}

impl<C, CF, CR, L, P, R, RP, F> SwapServer<C, CF, CR, L, P, R, RP, F>
where
    C: ChainClient,
    CF: ChainFilterService,
    CR: ChainRepository,
    L: LightningClient,
    P: PrivateKeyProvider,
    R: SwapRepository,
    RP: RandomProvider,
    F: FeeEstimator,
{
    pub fn new(params: SwapServerParams<C, CF, CR, L, P, R, RP, F>) -> Self {
        SwapServer {
            network: params.network,
            max_swap_amount_sat: params.max_swap_amount_sat,
            min_confirmations: params.min_confirmations,
            min_claim_blocks: params.min_claim_blocks,
            min_viable_cltv: params.min_viable_cltv,
            pay_fee_limit_base_msat: params.pay_fee_limit_base_msat,
            pay_fee_limit_ppm: params.pay_fee_limit_ppm,
            pay_timeout_seconds: params.pay_timeout_seconds,
            chain_client: params.chain_service,
            chain_filter_service: params.chain_filter_service,
            chain_repository: params.chain_repository,
            lightning_client: params.lightning_client,
            random_provider: params.random_provider,
            swap_service: params.swap_service,
            swap_repository: params.swap_repository,
            fee_estimator: params.fee_estimator,
        }
    }

    async fn get_swap_parameters(&self) -> Result<SwapParameters, Status> {
        let fee_estimate = self
            .fee_estimator
            .estimate_fee(MIN_SWAP_AMOUNT_CONF_TARGET)
            .await?;
        // Assume a transaction weight of 1000.
        let min_utxo_amount_sat = (fee_estimate.sat_per_kw as u64) * 3 / 2;

        Ok(SwapParameters {
            max_swap_amount_sat: self.max_swap_amount_sat,
            min_swap_amount_sat: min_utxo_amount_sat,
            min_utxo_amount_sat,
        })
    }
}
#[tonic::async_trait]
impl<C, CF, CR, L, P, R, RP, F> Swapper for SwapServer<C, CF, CR, L, P, R, RP, F>
where
    C: ChainClient + Debug + Send + Sync + 'static,
    CF: ChainFilterService + Debug + Send + Sync + 'static,
    CR: ChainRepository + Debug + Send + Sync + 'static,
    L: LightningClient + Debug + Send + Sync + 'static,
    P: PrivateKeyProvider + Debug + Send + Sync + 'static,
    R: SwapRepository + Debug + Send + Sync + 'static,
    RP: RandomProvider + Debug + Send + Sync + 'static,
    F: FeeEstimator + Debug + Send + Sync + 'static,
{
    #[instrument(skip(self), level = "debug")]
    async fn create_swap(
        &self,
        request: Request<CreateSwapRequest>,
    ) -> Result<Response<CreateSwapResponse>, Status> {
        debug!("create_swap request");
        let req = request.into_inner();
        let payer_pubkey = PublicKey::from_slice(&req.refund_pubkey).map_err(|_| {
            trace!("got invalid refund_pubkey");
            Status::invalid_argument("invalid refund_pubkey")
        })?;
        let hash = Hash::from_slice(&req.hash).map_err(|_| {
            trace!("got invalid hash");
            Status::invalid_argument("invalid hash")
        })?;

        // Get a fee estimate for the next block to account for worst case fees.
        let current_height = self.chain_client.get_blockheight().await?;

        let swap = self
            .swap_service
            .create_swap(payer_pubkey, hash, current_height)
            .map_err(|e| {
                error!("failed to create swap: {:?}", e);
                Status::internal("internal error")
            })?;

        // TODO: These need to go in a transaction.
        self.chain_repository
            .add_watch_address(&swap.public.address)
            .await?;
        self.swap_repository.add_swap(&swap).await?;

        info!(
            hash = field::display(&hash),
            address = field::display(&swap.public.address),
            "new swap created"
        );

        let parameters = self.get_swap_parameters().await?;
        Ok(Response::new(CreateSwapResponse {
            address: swap.public.address.to_string(),
            claim_pubkey: swap.public.claim_pubkey.serialize().to_vec(),
            lock_time: swap.public.lock_time.into(),
            parameters: Some(parameters),
        }))
    }

    #[instrument(skip(self), level = "debug")]
    async fn pay_swap(
        &self,
        request: Request<PaySwapRequest>,
    ) -> Result<Response<PaySwapResponse>, Status> {
        debug!("pay_swap request");
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

        let parameters = self.get_swap_parameters().await?;
        if amount_sat > parameters.max_swap_amount_sat {
            trace!(
                amount_sat,
                max_swap_amount_sat = parameters.max_swap_amount_sat,
                "invoice amount exceeds max swap amount"
            );
            return Err(Status::invalid_argument("amount exceeds max swap amount"));
        }

        if amount_sat < parameters.min_swap_amount_sat {
            trace!(
                amount_sat,
                min_swap_amount_sat = parameters.min_swap_amount_sat,
                "invoice amount is below min swap amount"
            );
            return Err(Status::invalid_argument("amount is below min swap amount"));
        }

        let hash = invoice.payment_hash();
        let swap_state = self.swap_repository.get_swap_by_hash(hash).await?;
        if swap_state.preimage.is_some() {
            trace!("swap already had preimage");
            return Err(Status::failed_precondition("swap already paid"));
        }

        let min_final_cltv_expiry_delta: u32 = invoice
            .min_final_cltv_expiry_delta()
            .try_into()
            .map_err(|_| {
                trace!("min_final_cltv_expiry_delta exceeds u32::MAX");
                Status::invalid_argument("min_final_cltv_expiry_delta too high")
            })?;

        let txos = self
            .chain_repository
            .get_txos_for_address(&swap_state.swap.public.address)
            .await?;

        if txos.is_empty() {
            trace!("swap has no utxos");
            return Err(Status::failed_precondition("no utxos found"));
        }

        let min_confirmation_height = match txos.iter().map(|txo| txo.block_height).min() {
            Some(m) => m,
            None => {
                error!("swap had txos but no confirmations");
                return Err(Status::failed_precondition("no utxos found"));
            }
        };

        let current_height = self.chain_client.get_blockheight().await?;
        let blocks_left = match swap_state.blocks_left(min_confirmation_height, current_height) {
            blocks_left if blocks_left < 0 => {
                return Err(Status::failed_precondition("swap expired"))
            }
            blocks_left => blocks_left as u32,
        }
        .saturating_sub(self.min_claim_blocks);

        if blocks_left == 0
            || blocks_left.saturating_sub(min_final_cltv_expiry_delta) < self.min_viable_cltv
        {
            trace!(
                blocks_left,
                min_viable_cltv = self.min_viable_cltv,
                "payout blocks left too low"
            );
            return Err(Status::failed_precondition("swap expired"));
        }

        let txos = txos
            .into_iter()
            .filter(|txo| {
                let confirmations = txo.confirmations(current_height);
                if confirmations < self.min_confirmations {
                    debug!(
                        outpoint = field::display(txo.outpoint),
                        confirmations,
                        min_confirmations = self.min_confirmations,
                        "utxo has less than min confirmations"
                    );
                    return false;
                }

                if txo.tx_out.value.to_sat() < parameters.min_utxo_amount_sat {
                    debug!(
                        outpoint = field::display(txo.outpoint),
                        utxo_amount_sat = txo.tx_out.value.to_sat(),
                        min_utxo_amount_sat = parameters.min_utxo_amount_sat,
                        "utxo value is below min_utxo_amount_sat"
                    );
                    return false;
                }

                trace!(
                    outpoint = field::display(txo.outpoint),
                    confirmations,
                    min_confirmations = self.min_confirmations,
                    "utxo has correct amount of confirmations"
                );
                true
            })
            .collect::<Vec<Txo>>();

        // TODO: Filter utxos on sync?
        let txos = match self.chain_filter_service.filter_txos(txos.clone()).await {
            Ok(txos) => txos,
            Err(e) => {
                error!("failed to filter utxos: {:?}", e);
                txos
            }
        };

        // TODO: Add ability to charge a fee?
        // Sum the utxo amounts.
        let amount_sum_sat = txos
            .iter()
            .fold(0u64, |sum, utxo| sum + utxo.tx_out.value.to_sat());
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
        // is claimable within reasonable time.
        let fee_estimate = self.fee_estimator.estimate_fee(6).await?;
        let fake_address = Address::p2wpkh(
            &CompressedPublicKey::from_slice(&[0x02; 33]).map_err(|e| {
                error!("failed to create fake pubkey: {:?}", e);
                Status::internal("internal error")
            })?,
            self.network,
        );

        // If the claim tx can be created, this is a valid swap.
        self.swap_service
            .create_claim_tx(
                &txos
                    .iter()
                    .map(|utxo| ClaimableUtxo {
                        swap: swap_state.swap.clone(),
                        utxo: utxo.clone(),
                        paid_with_request: None,
                        preimage: FAKE_PREIMAGE,
                    })
                    .collect::<Vec<_>>(),
                &fee_estimate,
                current_height,
                fake_address,
            )
            .map_err(|e| {
                debug!("could not create valid fake claim tx: {:?}", e);
                Status::failed_precondition("value too low")
            })?;

        // Store the payment attempt to ensure not 'too many' utxos are claimed
        // on claim if a user accidentally sends multiple utxos to the same
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
        match self
            .swap_repository
            .lock_swap_payment(&swap_state.swap, &label)
            .await
        {
            Ok(_) => debug!("locked swap for payment"),
            Err(LockSwapError::AlreadyLocked) => {
                return Err(Status::failed_precondition("swap is locked"))
            }
            Err(e) => {
                error!("failed to lock swap for payment: {:?}", e);
                return Err(Status::internal("internal error"));
            }
        };
        self.swap_repository
            .add_payment_attempt(&PaymentAttempt {
                amount_msat,
                creation_time: now,
                label: label.clone(),
                destination: invoice.get_payee_pub_key(),
                payment_request: req.payment_request.clone(),
                payment_hash: swap_state.swap.public.hash,
                utxos: txos,
            })
            .await?;

        // Pay the user. After the payment succeeds, we will have paid the
        // funds, but not claimed anything onchain yet. That will happen in the
        // claim module.
        // TODO: Add a maximum fee here?
        let fee_limit_msat = self.pay_fee_limit_base_msat
            + amount_msat
                .saturating_mul(self.pay_fee_limit_ppm)
                .saturating_div(1_000_000);
        debug!("about to pay");
        let pay_result = self
            .lightning_client
            .pay(PaymentRequest {
                bolt11: req.payment_request,
                cltv_limit: blocks_left,
                payment_hash: *hash,
                label: label.clone(),
                fee_limit_msat,
                timeout_seconds: self.pay_timeout_seconds,
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
                    result = field::debug(&pay_result),
                    "failed to persist pay result: {:?}",
                    e
                );
            }
        };

        let _ = self
            .swap_repository
            .unlock_swap_payment(&swap_state.swap, &label)
            .await;
        let response = match pay_result {
            PaymentResult::Success { preimage: _ } => {
                info!(
                    label = field::display(&label),
                    hash = field::display(hash),
                    address = field::display(swap_state.swap.public.address),
                    "successfully paid"
                );
                PaySwapResponse::default()
            }
            PaymentResult::Failure { error } => {
                info!("payment failed with: {}", error);
                return Err(Status::unknown("payment failed"));
            }
        };

        Ok(Response::new(response))
    }

    #[instrument(skip(self), level = "debug")]
    async fn refund_swap(
        &self,
        request: Request<RefundSwapRequest>,
    ) -> Result<Response<RefundSwapResponse>, Status> {
        debug!("refund_swap request");
        let req = request.into_inner();
        let tx = Transaction::consensus_decode(&mut req.transaction.as_slice()).map_err(|e| {
            trace!("got invalid transaction: {:?}", e);
            Status::invalid_argument("invalid transaction")
        })?;
        let their_pub_nonce = MusigPubNonce::from_slice(&req.pub_nonce).map_err(|e| {
            trace!("got invalid pub nonce: {:?}", e);
            Status::invalid_argument("invalid pub_nonce")
        })?;
        let input_index = req.input_index as usize;
        if input_index >= tx.input.len() {
            trace!("got input_index above tx input length");
            return Err(Status::invalid_argument("invalid input_index"));
        }
        let address = req
            .address
            .parse::<Address<NetworkUnchecked>>()
            .map_err(|e| {
                trace!("could not parse address: {:?}", e);
                Status::invalid_argument("invalid address")
            })?
            .require_network(self.network)
            .map_err(|e| {
                trace!("address for wrong network: {:?}", e);
                Status::invalid_argument("invalid address")
            })?;

        let swap = self.swap_repository.get_swap_by_address(&address).await?;

        let prevout_futures = tx.input.iter().map(|vin| async {
            let tx = self
                .chain_client
                .get_transaction(&vin.previous_output.txid)
                .await
                .map_err(|e| {
                    trace!("refund tx input not found: {:?}", e);
                    Status::invalid_argument("invalid transaction input")
                })?;
            if vin.previous_output.vout as usize >= tx.output.len() {
                return Err(Status::invalid_argument("invalid transaction input"));
            }

            Ok(tx.output[vin.previous_output.vout as usize].clone())
        });
        let prevouts: Vec<TxOut> = join_all(prevout_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        assert!(
            prevouts.len() == tx.input.len(),
            "expected prevouts len to be equal to tx input len."
        );

        // Double check this is signing a refund to an actual swap output.
        let refund_prevout = &prevouts[input_index];
        let refund_prevout_address =
            Address::from_script(&refund_prevout.script_pubkey, self.network).map_err(|e| {
                trace!("refund tx input is not a valid address: {:?}", e);
                Status::invalid_argument("invalid transaction input")
            })?;

        if refund_prevout_address != address {
            return Err(Status::invalid_argument("invalid transaction input"));
        }

        let (partial_signature, our_pub_nonce) = self
            .swap_service
            .partial_sign_refund_tx(&swap.swap, tx, prevouts, input_index, their_pub_nonce)
            .map_err(|e| {
                error!("failed to sign refund transaction: {:?}", e);
                Status::internal("internal error")
            })?;

        let refund_id = hex::encode(self.random_provider.rnd_32()?);

        // Ensure this swap is not used for paying out at the moment, also
        // prevent payouts from happening in the future. Note this will never be
        // unlocked, because the user may steal funds if it's ever paid out. The
        // user _can_ create a new refund later, however.
        match self
            .swap_repository
            .lock_swap_refund(&swap.swap, &refund_id)
            .await
        {
            Ok(_) => debug!("locked swap for refund."),
            Err(LockSwapError::AlreadyLocked) => {
                return Err(Status::failed_precondition("swap is locked"))
            }
            Err(e) => {
                error!("failed to lock swap for refund: {:?}", e);
                return Err(Status::internal("internal error"));
            }
        };

        match self
            .lightning_client
            .has_pending_or_complete_payment(&swap.swap.public.hash)
            .await
        {
            Ok(false) => {}
            Ok(true) => {
                let _ = self
                    .swap_repository
                    .unlock_swap_refund(&swap.swap, &refund_id)
                    .await;
                return Err(Status::failed_precondition("swap is locked"));
            }
            Err(e) => {
                error!("failed to check for pending or complete payment: {:?}", e);
                let _ = self
                    .swap_repository
                    .unlock_swap_refund(&swap.swap, &refund_id)
                    .await;
                return Err(Status::internal("internal error"));
            }
        }

        Ok(Response::new(RefundSwapResponse {
            partial_signature: partial_signature.serialize().to_vec(),
            pub_nonce: our_pub_nonce.serialize().to_vec(),
        }))
    }

    async fn swap_parameters(
        &self,
        _request: Request<SwapParametersRequest>,
    ) -> Result<Response<SwapParametersResponse>, Status> {
        let parameters = self.get_swap_parameters().await?;
        Ok(Response::new(SwapParametersResponse {
            parameters: Some(parameters),
        }))
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

impl From<GetSwapsError> for Status {
    fn from(value: GetSwapsError) -> Self {
        match value {
            GetSwapsError::NotFound => {
                trace!("swap not found");
                Status::not_found("swap not found")
            }
            GetSwapsError::General(e) => {
                error!("failed to get swap: {:?}", e);
                Status::internal("internal error")
            }
            GetSwapsError::InvalidPreimage => {
                error!("got invalid preimage");
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
            ChainError::BlockNotFound => {
                error!("got block not found error");
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

impl From<LightningError> for Status {
    fn from(value: LightningError) -> Self {
        debug!("payment failed: {:?}", value);
        Status::unknown("payment failed")
    }
}

impl From<ChainRepositoryError> for Status {
    fn from(value: ChainRepositoryError) -> Self {
        match value {
            ChainRepositoryError::MultipleTips => {
                error!("chain has multiple tips");
                Status::internal("internal error")
            }
            ChainRepositoryError::General(e) => {
                error!("failed to get chain data: {:?}", e);
                Status::internal("internal error")
            }
        }
    }
}

impl From<RandomError> for Status {
    fn from(value: RandomError) -> Self {
        error!("random error: {:?}", value);
        Status::unknown("internal error")
    }
}
