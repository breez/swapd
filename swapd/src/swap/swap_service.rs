use std::time::SystemTime;

use super::privkey_provider::{PrivateKeyError, PrivateKeyProvider};
use crate::chain::{FeeEstimate, Utxo};
use bitcoin::{
    absolute::LockTime,
    hashes::{ripemd160, sha256, Hash},
    opcodes::all::{OP_CHECKSIG, OP_CHECKSIGVERIFY, OP_CLTV, OP_EQUALVERIFY, OP_HASH160},
    secp256k1::{All, Message, PublicKey, Secp256k1, SecretKey},
    sighash::{self, Prevouts, SighashCache},
    taproot::{LeafVersion, Signature, TaprootBuilder, TaprootSpendInfo},
    transaction::Version,
    Address, Amount, CompressedPublicKey, Network, Script, ScriptBuf, Sequence, TapLeafHash,
    TapSighashType, Transaction, TxIn, TxOut, Weight, Witness, XOnlyPublicKey,
};
use secp256k1::musig::{
    MusigAggNonce, MusigKeyAggCache, MusigPartialSignature, MusigPubNonce, MusigSession,
    MusigSessionId,
};
use thiserror::Error;
use tracing::{error, instrument, trace};

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

#[derive(Debug, Error)]
pub enum SwapError {
    #[error("private key: {0}")]
    PrivateKey(PrivateKeyError),
    #[error("invalid block height: {0}")]
    InvalidBlockHeight(bitcoin::locktime::absolute::ConversionError),
    #[error("fake address error")]
    FakeAddress,
    #[error("taproot: {0}")]
    Taproot(TaprootError),
    #[error("invalid weight")]
    InvalidWeight,
    #[error("amount too low")]
    AmountTooLow,
    #[error("conversion error")]
    Conversion,
    #[error("musig tweak: {0}")]
    MusigTweak(secp256k1::musig::MusigTweakErr),
    #[error("nonce gen: {0}")]
    NonceGen(secp256k1::musig::MusigNonceGenError),
    #[error("sign: {0}")]
    Sign(secp256k1::musig::MusigSignError),
}

#[derive(Debug)]
pub struct SwapService<P>
where
    P: PrivateKeyProvider,
{
    dust_limit_sat: u64,
    network: Network,
    secp: Secp256k1<All>,
    // NOTE: Remove once the bitcoin crate contains the musig module.
    musig_secp: secp256k1::Secp256k1<secp256k1::All>,
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
            musig_secp: secp256k1::Secp256k1::new(),
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
    ) -> Result<Swap, SwapError> {
        let creation_time = SystemTime::now();
        let claim_privkey = self.privkey_provider.new_private_key()?;
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
                SwapError::FakeAddress
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
    ) -> Result<Transaction, SwapError> {
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
            .ok_or(SwapError::InvalidWeight)?;
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
            return Err(SwapError::AmountTooLow);
        }
        tx.output[0].value = Amount::from_sat(value_after_fees_sat);

        let prevouts: Vec<TxOut> = claimables.iter().map(|u| u.utxo.tx_out.clone()).collect();
        let prevouts = Prevouts::All(&prevouts);
        for (n, c) in claimables.iter().enumerate() {
            let leaf_hash =
                TapLeafHash::from_script(&c.swap.public.claim_script, LeafVersion::TapScript);

            let mut sighasher = sighash::SighashCache::new(&tx);
            let sighash = sighasher
                .taproot_script_spend_signature_hash(
                    n,
                    &prevouts,
                    leaf_hash,
                    TapSighashType::Default,
                )
                .map_err(TaprootError::TaprootSighash)?;

            let rnd = self.privkey_provider.new_private_key()?.secret_bytes();
            let msg = Message::from(sighash);
            let signature = self.secp.sign_schnorr_with_aux_rand(
                &msg,
                &c.swap.private.claim_privkey.keypair(&self.secp),
                &rnd,
            );
            let signature = Signature {
                signature,
                sighash_type: TapSighashType::Default,
            };
            let control_block = self
                .taproot_spend_info(&c.swap)?
                .control_block(&(c.swap.public.claim_script.clone(), LeafVersion::TapScript))
                .ok_or(TaprootError::MissingControlBlock)?;
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

    pub fn partial_sign_refund_tx(
        &self,
        swap: &Swap,
        tx: Transaction,
        prevouts: Vec<TxOut>,
        input_index: usize,
        their_pub_nonce: MusigPubNonce,
    ) -> Result<(MusigPartialSignature, MusigPubNonce), SwapError> {
        let tweak = self.taproot_spend_info(swap)?.tap_tweak();
        let tweak_scalar = tweak.to_scalar();

        // TODO: Remove conversion once bitcoin crate contains musig module.
        let tweak_scalar = secp256k1::Scalar::from_be_bytes(tweak_scalar.to_be_bytes())?;
        let claim_pubkey = secp256k1::PublicKey::from_slice(&swap.public.claim_pubkey.serialize())?;
        let claim_privkey =
            secp256k1::SecretKey::from_byte_array(&swap.private.claim_privkey.secret_bytes())?;

        let mut key_agg_cache = self.key_agg_cache(swap)?;
        let _ = key_agg_cache.pubkey_xonly_tweak_add(&self.musig_secp, &tweak_scalar)?;
        let session_id = MusigSessionId::assume_unique_per_nonce_gen(
            self.privkey_provider.new_private_key()?.secret_bytes(),
        );

        let mut sighasher = SighashCache::new(tx);
        let prevouts = Prevouts::All(&prevouts);
        let sighash = sighasher
            .taproot_key_spend_signature_hash(input_index, &prevouts, TapSighashType::Default)
            .map_err(TaprootError::TaprootSighash)?;
        let msg = secp256k1::Message::from_digest(sighash.to_byte_array());
        let extra_rand = self.privkey_provider.new_private_key()?.secret_bytes();

        let (our_sec_nonce, our_pub_nonce) = key_agg_cache.nonce_gen(
            &self.musig_secp,
            session_id,
            claim_pubkey,
            msg,
            Some(extra_rand),
        )?;
        let agg_nonce = MusigAggNonce::new(&self.musig_secp, &[&our_pub_nonce, &their_pub_nonce]);
        let musig_session = MusigSession::new(&self.musig_secp, &key_agg_cache, agg_nonce, msg);

        let partial_sig = musig_session.partial_sign(
            &self.musig_secp,
            our_sec_nonce,
            &claim_privkey.keypair(&self.musig_secp),
            &key_agg_cache,
        )?;
        Ok((partial_sig, our_pub_nonce))
    }

    fn key_agg_cache(&self, swap: &Swap) -> Result<MusigKeyAggCache, TaprootError> {
        // TODO: Remove conversion once bitcoin crate contains musig module.
        let cp = secp256k1::PublicKey::from_slice(&swap.public.claim_pubkey.serialize())?;
        let rp = secp256k1::PublicKey::from_slice(&swap.public.refund_pubkey.serialize())?;
        Ok(MusigKeyAggCache::new(&self.musig_secp, &[&cp, &rp]))
    }

    fn taproot_spend_info(&self, swap: &Swap) -> Result<TaprootSpendInfo, TaprootError> {
        let m = self.key_agg_cache(swap)?;
        let internal_key = m.agg_pk();

        // TODO: Remove conversion once bitcoin crate contains musig module.
        let internal_key = XOnlyPublicKey::from_slice(&internal_key.serialize())?;

        // claim and refund scripts go in a taptree.
        Ok(TaprootBuilder::new()
            .add_leaf(1, swap.public.claim_script.clone())?
            .add_leaf(1, swap.public.refund_script.clone())?
            .finalize(&self.secp, internal_key)?)
    }
}

#[derive(Debug, Error)]
pub enum TaprootError {
    #[error("secp256k1: {0}")]
    Secp256k1(secp256k1::Error),
    #[error("bitcoin secp256k1: {0}")]
    BitcoinSecp256k1(bitcoin::secp256k1::Error),
    #[error("taproot builder: {0}")]
    TaprootBuilder(bitcoin::taproot::TaprootBuilderError),
    #[error("could not finalize taproot spend info")]
    TaprootSpend(bitcoin::taproot::TaprootBuilder),
    #[error("taproot: {0}")]
    TaprootSighash(bitcoin::sighash::TaprootError),
    #[error("missing control block")]
    MissingControlBlock,
}

impl From<secp256k1::Error> for TaprootError {
    fn from(value: secp256k1::Error) -> Self {
        TaprootError::Secp256k1(value)
    }
}

impl From<bitcoin::secp256k1::Error> for TaprootError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        TaprootError::BitcoinSecp256k1(value)
    }
}

impl From<bitcoin::taproot::TaprootBuilderError> for TaprootError {
    fn from(value: bitcoin::taproot::TaprootBuilderError) -> Self {
        TaprootError::TaprootBuilder(value)
    }
}

impl From<bitcoin::taproot::TaprootBuilder> for TaprootError {
    fn from(value: bitcoin::taproot::TaprootBuilder) -> Self {
        TaprootError::TaprootSpend(value)
    }
}

impl From<PrivateKeyError> for SwapError {
    fn from(value: PrivateKeyError) -> Self {
        SwapError::PrivateKey(value)
    }
}

impl From<bitcoin::locktime::absolute::ConversionError> for SwapError {
    fn from(value: bitcoin::locktime::absolute::ConversionError) -> Self {
        SwapError::InvalidBlockHeight(value)
    }
}

impl From<TaprootError> for SwapError {
    fn from(value: TaprootError) -> Self {
        SwapError::Taproot(value)
    }
}

impl From<secp256k1::scalar::OutOfRangeError> for SwapError {
    fn from(value: secp256k1::scalar::OutOfRangeError) -> Self {
        error!("conversion error, out of range: {:?}", value);
        SwapError::Conversion
    }
}

impl From<secp256k1::Error> for SwapError {
    fn from(value: secp256k1::Error) -> Self {
        error!("conversion error, secp: {:?}", value);
        SwapError::Conversion
    }
}

impl From<secp256k1::musig::MusigTweakErr> for SwapError {
    fn from(value: secp256k1::musig::MusigTweakErr) -> Self {
        SwapError::MusigTweak(value)
    }
}

impl From<secp256k1::musig::MusigNonceGenError> for SwapError {
    fn from(value: secp256k1::musig::MusigNonceGenError) -> Self {
        SwapError::NonceGen(value)
    }
}

impl From<secp256k1::musig::MusigSignError> for SwapError {
    fn from(value: secp256k1::musig::MusigSignError) -> Self {
        SwapError::Sign(value)
    }
}
