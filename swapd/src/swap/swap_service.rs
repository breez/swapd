use std::time::SystemTime;

use bitcoin::{
    absolute::{self, LockTime},
    hashes::{ripemd160, sha256, Hash},
    key::Secp256k1,
    opcodes::all::{OP_CHECKSIG, OP_CSV, OP_DROP, OP_ELSE, OP_ENDIF, OP_EQUAL, OP_HASH160, OP_IF},
    secp256k1::{All, Message, SecretKey},
    sighash::{self, EcdsaSighashType},
    Address, Network, PrivateKey, PublicKey, Script, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Weight, Witness,
};
use tracing::{debug, field, instrument, trace};

use crate::chain::{FeeEstimate, Utxo};

use super::privkey_provider::PrivateKeyProvider;

// TODO: Verify this size
const REDEEM_INPUT_WITNESS_SIZE: usize = 1 + 1 + 73 + 1 + 32 + 1 + 100;

#[derive(Debug)]
pub struct SwapState {
    pub swap: Swap,
    pub preimage: Option<[u8; 32]>,
}

#[derive(Clone, Debug)]
pub struct Swap {
    pub creation_time: SystemTime,
    pub public: SwapPublicData,
    pub private: SwapPrivateData,
}

#[derive(Clone, Debug)]
pub struct SwapPublicData {
    pub payer_pubkey: PublicKey,
    pub swapper_pubkey: PublicKey,
    pub hash: sha256::Hash,
    pub script: ScriptBuf,
    pub address: Address,
    pub lock_time: u32,
}

#[derive(Clone)]
pub struct SwapPrivateData {
    pub swapper_privkey: PrivateKey,
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
}

#[derive(Debug)]
pub enum CreateRedeemTxError {
    InvalidBlockHeight,
    InvalidWeight,
    InvalidSigningData,
    InvalidMessage,
    InvalidSecretKey,
    AmountTooLow,
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
        payer_pubkey: PublicKey,
        hash: sha256::Hash,
    ) -> Result<Swap, CreateSwapError> {
        let creation_time = SystemTime::now();
        let swapper_privkey = self.privkey_provider.new_private_key().map_err(|e| {
            debug!("error creating private key: {:?}", e);
            CreateSwapError::PrivateKeyError
        })?;
        let swapper_pubkey = swapper_privkey.public_key(&self.secp);

        let lock_time = self.lock_time;

        let script = Script::builder()
            .push_opcode(OP_HASH160)
            .push_slice(ripemd160::Hash::hash(hash.as_byte_array()).as_byte_array())
            .push_opcode(OP_EQUAL)
            .push_opcode(OP_IF)
            .push_key(&swapper_pubkey)
            .push_opcode(OP_ELSE)
            .push_int(lock_time as i64)
            .push_opcode(OP_CSV)
            .push_opcode(OP_DROP)
            .push_key(&payer_pubkey)
            .push_opcode(OP_ENDIF)
            .push_opcode(OP_CHECKSIG)
            .into_script();

        let address = Address::p2wsh(&script, self.network);

        Ok(Swap {
            creation_time,
            public: SwapPublicData {
                payer_pubkey,
                swapper_pubkey,
                hash,
                script,
                address,
                lock_time,
            },
            private: SwapPrivateData { swapper_privkey },
        })
    }

    #[instrument(level = "trace", skip(self))]
    pub fn create_redeem_tx(
        &self,
        swap: &Swap,
        utxos: &[Utxo],
        fee: &FeeEstimate,
        current_height: u64,
        preimage: &[u8; 32],
        destination_address: Address,
    ) -> Result<Transaction, CreateRedeemTxError> {
        let total_value = utxos.iter().fold(0u64, |sum, utxo| sum + utxo.amount_sat);
        let mut tx = Transaction {
            version: 2,
            lock_time: LockTime::from_height(current_height as u32)?,
            input: utxos
                .iter()
                .map(|utxo| TxIn {
                    previous_output: utxo.outpoint,
                    script_sig: ScriptBuf::default(),
                    sequence: Sequence::ZERO,
                    witness: Witness::default(),
                })
                .collect(),
            output: vec![TxOut {
                script_pubkey: destination_address.into(),
                value: total_value,
            }],
        };

        let weight = tx
            .weight()
            .checked_add(Weight::from_wu(
                (REDEEM_INPUT_WITNESS_SIZE * tx.input.len()) as u64,
            ))
            .ok_or(CreateRedeemTxError::InvalidWeight)?;
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
            return Err(CreateRedeemTxError::AmountTooLow);
        }
        tx.output[0].value = value_after_fees_sat;

        let mut sighash_cache = sighash::SighashCache::new(&tx);
        let mut inputs = Vec::new();
        for (n, utxo) in utxos.iter().enumerate() {
            let mut witness_sighash: Vec<u8> = Vec::new();
            sighash_cache.segwit_encode_signing_data_to(
                &mut witness_sighash,
                n,
                &swap.public.script,
                utxo.amount_sat,
                EcdsaSighashType::All,
            )?;
            let msg = Message::from_slice(&witness_sighash)?;
            let secret_key = SecretKey::from_slice(&swap.private.swapper_privkey.to_bytes())?;
            let signature = self
                .secp
                .sign_ecdsa(&msg, &secret_key)
                .serialize_der()
                .to_vec();
            let witness = vec![signature, preimage.to_vec(), swap.public.script.to_bytes()];
            let mut input = tx.input[n].clone();
            input.witness = witness.into();
            inputs.push(input);
        }

        tx.input = inputs;
        // TODO: Verify weight and fee are correct.

        Ok(tx)
    }
}

impl From<absolute::Error> for CreateRedeemTxError {
    fn from(_value: absolute::Error) -> Self {
        CreateRedeemTxError::InvalidBlockHeight
    }
}

impl From<sighash::Error> for CreateRedeemTxError {
    fn from(_value: sighash::Error) -> Self {
        CreateRedeemTxError::InvalidSigningData
    }
}

impl From<bitcoin::secp256k1::Error> for CreateRedeemTxError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        trace!(secp256k1_error = field::debug(value));
        match value {
            bitcoin::secp256k1::Error::IncorrectSignature => todo!(),
            bitcoin::secp256k1::Error::InvalidMessage => CreateRedeemTxError::InvalidMessage,
            bitcoin::secp256k1::Error::InvalidPublicKey => todo!(),
            bitcoin::secp256k1::Error::InvalidSignature => todo!(),
            bitcoin::secp256k1::Error::InvalidSecretKey => todo!(),
            bitcoin::secp256k1::Error::InvalidSharedSecret => CreateRedeemTxError::InvalidSecretKey,
            bitcoin::secp256k1::Error::InvalidRecoveryId => todo!(),
            bitcoin::secp256k1::Error::InvalidTweak => todo!(),
            bitcoin::secp256k1::Error::NotEnoughMemory => todo!(),
            bitcoin::secp256k1::Error::InvalidPublicKeySum => todo!(),
            bitcoin::secp256k1::Error::InvalidParityValue(_) => todo!(),
        }
    }
}
