use std::time::SystemTime;

use super::privkey_provider::PrivateKeyProvider;
use crate::chain::{FeeEstimate, Utxo};
use bitcoin::{
    absolute::LockTime,
    hashes::{ripemd160, sha256, Hash},
    key::Secp256k1,
    opcodes::all::{OP_CHECKSIG, OP_CHECKSIGVERIFY, OP_CLTV, OP_EQUALVERIFY, OP_HASH160},
    secp256k1::{All, Message, PublicKey, SecretKey},
    sighash::{self, Prevouts},
    taproot::{LeafVersion, Signature, TaprootBuilder, TaprootSpendInfo},
    transaction::Version,
    Address, Amount, CompressedPublicKey, Network, Script, ScriptBuf, Sequence, TapLeafHash,
    TapSighashType, Transaction, TxIn, TxOut, Weight, Witness,
};
use thiserror::Error;
use tracing::{error, field, instrument, trace};

// TODO: fix for taproot
const CLAIM_INPUT_WITNESS_SIZE: usize = 1 + 1 + 73 + 1 + 32 + 1 + 100;

#[derive(Clone, Debug)]
pub struct ClaimableUtxo {
    pub swap: Swap,
    pub utxo: Utxo,
    pub paid_with_request: Option<String>,
    pub preimage: [u8; 32],
}

#[derive(Debug)]
pub struct SwapState {
    pub swap: Swap,
    pub preimage: Option<[u8; 32]>,
}

impl SwapState {
    pub fn blocks_left(&self, current_height: u64) -> i32 {
        self.swap.blocks_left(current_height)
    }
}

#[derive(Clone, Debug)]
pub struct Swap {
    pub creation_time: SystemTime,
    pub public: SwapPublicData,
    pub private: SwapPrivateData,
}

impl Swap {
    pub fn blocks_left(&self, current_height: u64) -> i32 {
        (self.public.lock_height as i64 - current_height as i64) as i32
    }
}

#[derive(Clone, Debug)]
pub struct SwapPublicData {
    pub address: Address,
    pub claim_pubkey: PublicKey,
    pub claim_script: ScriptBuf,
    pub hash: sha256::Hash,
    pub lock_height: u32,
    pub refund_pubkey: PublicKey,
    pub refund_script: ScriptBuf,
}

#[derive(Clone)]
pub struct SwapPrivateData {
    pub claim_privkey: SecretKey,
}

impl std::fmt::Debug for SwapPrivateData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwapPrivateData")
            .field("swapper_privkey", &"redacted")
            .finish()
    }
}

#[derive(Debug)]
pub enum CreateSwapError {
    PrivateKeyError,
    InvalidBlockHeight,
    InvalidTransaction,
    Taproot(TaprootError),
}

#[derive(Debug, Error)]
pub enum CreateClaimTxError {
    #[error("invalid block height")]
    InvalidBlockHeight,
    #[error("invalid weight")]
    InvalidWeight,
    #[error("invalid signing data")]
    InvalidSigningData,
    #[error("invalid message")]
    InvalidMessage,
    #[error("invalid secret key")]
    InvalidSecretKey,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("not enough memory")]
    NotEnoughMemory,
    #[error("amount too low")]
    AmountTooLow,
    #[error("taproot error: {0}")]
    Taproot(TaprootError),
}

#[derive(Debug)]
pub struct SwapService<P>
where
    P: PrivateKeyProvider,
{
    dust_limit_sat: u64,
    network: Network,
    secp: Secp256k1<All>,
    privkey_provider: P,
    lock_time: u32,
}

impl<P> SwapService<P>
where
    P: PrivateKeyProvider,
{
    pub fn new(
        network: impl Into<Network>,
        privkey_provider: P,
        lock_time: u32,
        dust_limit_sat: u64,
    ) -> Self {
        Self {
            dust_limit_sat,
            network: network.into(),
            secp: Secp256k1::new(),
            privkey_provider,
            lock_time,
        }
    }

    #[instrument(level = "trace", skip(self))]
    pub fn create_swap(
        &self,
        refund_pubkey: PublicKey,
        hash: sha256::Hash,
        current_height: u64,
    ) -> Result<Swap, CreateSwapError> {
        let creation_time = SystemTime::now();
        let claim_privkey = self.privkey_provider.new_private_key().map_err(|e| {
            error!("error creating private key: {:?}", e);
            CreateSwapError::PrivateKeyError
        })?;
        let claim_pubkey = claim_privkey.public_key(&self.secp);
        let (x_only_claim_pubkey, _) = claim_pubkey.x_only_public_key();
        let (x_only_refund_pubkey, _) = refund_pubkey.x_only_public_key();
        let claim_script = Script::builder()
            .push_opcode(OP_HASH160)
            .push_slice(ripemd160::Hash::hash(hash.as_byte_array()).as_byte_array())
            .push_opcode(OP_EQUALVERIFY)
            .push_x_only_key(&x_only_claim_pubkey)
            .push_opcode(OP_CHECKSIG)
            .into_script();

        let timeout_block_height = current_height as u32 + self.lock_time;
        let lock_height = LockTime::from_height(timeout_block_height)?;
        let refund_script = Script::builder()
            .push_x_only_key(&x_only_refund_pubkey)
            .push_opcode(OP_CHECKSIGVERIFY)
            .push_lock_time(lock_height)
            .push_opcode(OP_CLTV)
            .into_script();

        let fake_address = Address::p2wpkh(
            &CompressedPublicKey::from_slice(&[0x02; 33]).map_err(|e| {
                error!("failed to create fake pubkey: {:?}", e);
                CreateSwapError::PrivateKeyError
            })?,
            self.network,
        );
        let mut swap = Swap {
            creation_time,
            public: SwapPublicData {
                address: fake_address,
                claim_pubkey,
                claim_script,
                hash,
                lock_height: lock_height.to_consensus_u32(),
                refund_pubkey,
                refund_script,
            },
            private: SwapPrivateData { claim_privkey },
        };

        let taproot_spend_info = self.taproot_spend_info(&swap)?;
        swap.public.address = Address::p2tr_tweaked(taproot_spend_info.output_key(), self.network);

        Ok(swap)
    }

    #[instrument(level = "trace", skip(self))]
    pub fn create_claim_tx(
        &self,
        claimables: &[ClaimableUtxo],
        fee: &FeeEstimate,
        current_height: u64,
        destination_address: Address,
    ) -> Result<Transaction, CreateClaimTxError> {
        // Sort by outpoint to reproducibly craft the same tx.
        let mut claimables = claimables.to_vec();
        claimables.sort_by(|a, b| a.utxo.outpoint.cmp(&b.utxo.outpoint));
        let total_value = claimables
            .iter()
            .fold(0u64, |sum, r| sum + r.utxo.tx_out.value.to_sat());
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::from_height(current_height as u32)?,
            input: claimables
                .iter()
                .map(|r| TxIn {
                    previous_output: r.utxo.outpoint,
                    script_sig: ScriptBuf::default(),
                    sequence: Sequence::ZERO,
                    witness: Witness::default(),
                })
                .collect(),
            output: vec![TxOut {
                script_pubkey: destination_address.into(),
                value: Amount::from_sat(total_value),
            }],
        };

        let weight = tx
            .weight()
            .checked_add(Weight::from_wu(
                (CLAIM_INPUT_WITNESS_SIZE * tx.input.len()) as u64,
            ))
            .ok_or(CreateClaimTxError::InvalidWeight)?;
        let fee_msat = weight.to_wu() * fee.sat_per_kw as u64;
        let fee_sat = (fee_msat + 999) / 1000;
        let value_after_fees_sat = total_value.saturating_sub(fee_sat);
        if value_after_fees_sat < self.dust_limit_sat {
            trace!(
                total_value,
                fee_sat,
                value_after_fees_sat,
                dust_limit_sat = self.dust_limit_sat
            );
            return Err(CreateClaimTxError::AmountTooLow);
        }
        tx.output[0].value = Amount::from_sat(value_after_fees_sat);

        let prevouts: Vec<TxOut> = claimables.iter().map(|u| u.utxo.tx_out.clone()).collect();
        let prevouts = Prevouts::All(&prevouts);
        for (n, c) in claimables.iter().enumerate() {
            let leaf_hash =
                TapLeafHash::from_script(&c.swap.public.claim_script, LeafVersion::TapScript);

            let mut sighasher = sighash::SighashCache::new(&tx);
            let sighash = sighasher.taproot_script_spend_signature_hash(
                n,
                &prevouts,
                leaf_hash,
                TapSighashType::All,
            )?;

            let rnd = self
                .privkey_provider
                .new_private_key()
                .map_err(|_| CreateClaimTxError::InvalidSecretKey)?
                .secret_bytes();
            let msg = Message::from(sighash);
            let signature = self.secp.sign_schnorr_with_aux_rand(
                &msg,
                &c.swap.private.claim_privkey.keypair(&self.secp),
                &rnd,
            );
            let signature = Signature {
                signature,
                sighash_type: TapSighashType::All,
            };
            let control_block = self
                .taproot_spend_info(&c.swap)?
                .control_block(&(c.swap.public.claim_script.clone(), LeafVersion::TapScript))
                .ok_or(CreateClaimTxError::InvalidSigningData)?;
            let witness = vec![
                signature.to_vec(),
                c.preimage.to_vec(),
                c.swap.public.claim_script.to_bytes(),
                control_block.serialize(),
            ];
            tx.input[n].witness = witness.into();
        }

        Ok(tx)
    }

    fn taproot_spend_info(&self, swap: &Swap) -> Result<TaprootSpendInfo, TaprootError> {
        // TODO: Replace musig2 library with rust-bitcoin musig2 when available.
        let cp = musig2::secp256k1::PublicKey::from_byte_array_compressed(
            &swap.public.claim_pubkey.serialize(),
        )?;
        let rp = musig2::secp256k1::PublicKey::from_byte_array_compressed(
            &swap.public.refund_pubkey.serialize(),
        )?;
        let m = musig2::KeyAggContext::new([cp, rp])?;
        let mp: musig2::secp256k1::PublicKey = m.aggregated_pubkey();
        let musig_pubkey = PublicKey::from_slice(&mp.serialize())?;
        let (internal_key, _) = musig_pubkey.x_only_public_key();

        // claim and refund scripts go in a taptree.
        Ok(TaprootBuilder::new()
            .add_leaf(1, swap.public.claim_script.clone())?
            .add_leaf(1, swap.public.refund_script.clone())?
            .finalize(&self.secp, internal_key)?)
    }
}

#[derive(Debug, Error)]
pub enum TaprootError {
    #[error("musig: {0}")]
    Musig(musig2::secp256k1::Error),
    #[error("key agg: {0}")]
    KeyAgg(musig2::errors::KeyAggError),
    #[error("key: {0}")]
    Key(bitcoin::secp256k1::Error),
    #[error("taproot builder: {0}")]
    TaprootBuilder(bitcoin::taproot::TaprootBuilderError),
    #[error("could not finalize taproot spend info")]
    Taproot(bitcoin::taproot::TaprootBuilder),
}

impl From<musig2::secp256k1::Error> for TaprootError {
    fn from(value: musig2::secp256k1::Error) -> Self {
        TaprootError::Musig(value)
    }
}

impl From<musig2::errors::KeyAggError> for TaprootError {
    fn from(value: musig2::errors::KeyAggError) -> Self {
        TaprootError::KeyAgg(value)
    }
}

impl From<bitcoin::secp256k1::Error> for TaprootError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        TaprootError::Key(value)
    }
}

impl From<bitcoin::taproot::TaprootBuilderError> for TaprootError {
    fn from(value: bitcoin::taproot::TaprootBuilderError) -> Self {
        TaprootError::TaprootBuilder(value)
    }
}

impl From<bitcoin::taproot::TaprootBuilder> for TaprootError {
    fn from(value: bitcoin::taproot::TaprootBuilder) -> Self {
        TaprootError::Taproot(value)
    }
}

impl From<bitcoin::absolute::ConversionError> for CreateSwapError {
    fn from(_value: bitcoin::absolute::ConversionError) -> Self {
        CreateSwapError::InvalidBlockHeight
    }
}

impl From<TaprootError> for CreateSwapError {
    fn from(value: TaprootError) -> Self {
        CreateSwapError::Taproot(value)
    }
}

impl From<TaprootError> for CreateClaimTxError {
    fn from(value: TaprootError) -> Self {
        CreateClaimTxError::Taproot(value)
    }
}

impl From<bitcoin::absolute::ConversionError> for CreateClaimTxError {
    fn from(_value: bitcoin::absolute::ConversionError) -> Self {
        CreateClaimTxError::InvalidBlockHeight
    }
}

impl From<bitcoin::blockdata::transaction::InputsIndexError> for CreateClaimTxError {
    fn from(_value: bitcoin::blockdata::transaction::InputsIndexError) -> Self {
        CreateClaimTxError::InvalidSigningData
    }
}

impl From<bitcoin::sighash::TaprootError> for CreateClaimTxError {
    fn from(value: bitcoin::sighash::TaprootError) -> Self {
        trace!(taproot_error = field::debug(&value));
        match value {
            sighash::TaprootError::InputsIndex(_) => CreateClaimTxError::InvalidMessage,
            sighash::TaprootError::SingleMissingOutput(_) => CreateClaimTxError::InvalidMessage,
            sighash::TaprootError::PrevoutsSize(_) => CreateClaimTxError::InvalidMessage,
            sighash::TaprootError::PrevoutsIndex(_) => CreateClaimTxError::InvalidMessage,
            sighash::TaprootError::PrevoutsKind(_) => CreateClaimTxError::InvalidMessage,
            sighash::TaprootError::InvalidSighashType(_) => CreateClaimTxError::InvalidMessage,
            _ => CreateClaimTxError::InvalidMessage,
        }
    }
}

impl From<bitcoin::secp256k1::Error> for CreateClaimTxError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        trace!(secp256k1_error = field::debug(value));
        match value {
            bitcoin::secp256k1::Error::IncorrectSignature => CreateClaimTxError::InvalidSignature,
            bitcoin::secp256k1::Error::InvalidMessage => CreateClaimTxError::InvalidMessage,
            bitcoin::secp256k1::Error::InvalidPublicKey => CreateClaimTxError::InvalidMessage,
            bitcoin::secp256k1::Error::InvalidSignature => CreateClaimTxError::InvalidSignature,
            bitcoin::secp256k1::Error::InvalidSecretKey => CreateClaimTxError::InvalidSecretKey,
            bitcoin::secp256k1::Error::InvalidSharedSecret => CreateClaimTxError::InvalidSecretKey,
            bitcoin::secp256k1::Error::InvalidRecoveryId => CreateClaimTxError::InvalidMessage,
            bitcoin::secp256k1::Error::InvalidTweak => CreateClaimTxError::InvalidMessage,
            bitcoin::secp256k1::Error::NotEnoughMemory => CreateClaimTxError::NotEnoughMemory,
            bitcoin::secp256k1::Error::InvalidPublicKeySum => CreateClaimTxError::InvalidMessage,
            bitcoin::secp256k1::Error::InvalidParityValue(_) => CreateClaimTxError::InvalidMessage,
            bitcoin::secp256k1::Error::InvalidEllSwift => CreateClaimTxError::InvalidMessage,
        }
    }
}
