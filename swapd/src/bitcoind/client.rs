use std::str::FromStr;

use bitcoin::{
    consensus::{deserialize, encode::serialize_hex, Decodable},
    hashes::{hex::FromHex, sha256d},
    Address, Block, BlockHash, Network, OutPoint, Transaction,
};
use reqwest::Method;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::trace;

use crate::chain::{BlockHeader, BroadcastError, ChainClient, ChainError};

use super::{
    EstimateSmartFeeResponse, GetBestBlockHashResponse, GetBlockCountResponse,
    GetBlockHeaderResponse, GetBlockResponse, GetRawTransactionResponse, RpcError, RpcRequest,
    RpcServerMessage, RpcServerMessageBody, SendRawTransactionResponse,
};

#[derive(Debug)]
pub struct BitcoindClient {
    address: String,
    user: String,
    password: String,
    counter: Mutex<u64>,
    network: Network,
}

#[derive(Debug, Error)]
pub(super) enum CallError {
    #[error("rpc error: {0:?}")]
    RpcError(RpcError),

    #[error("deserialize error: {0}")]
    Deserialize(serde_json::error::Error),

    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

impl From<reqwest::Error> for CallError {
    fn from(value: reqwest::Error) -> Self {
        CallError::General(Box::new(value))
    }
}

impl BitcoindClient {
    pub fn new(address: String, user: String, password: String, network: Network) -> Self {
        Self {
            address,
            user,
            password,
            counter: Mutex::new(0),
            network,
        }
    }

    async fn call<TParams, TResponse>(
        &self,
        method: impl Into<String>,
        params: TParams,
    ) -> Result<TResponse, CallError>
    where
        TParams: Serialize,
        TResponse: DeserializeOwned,
    {
        let method = method.into();
        trace!("calling {}", method);
        let client = reqwest::Client::new();
        let resp = client
            .request(Method::POST, &self.address)
            .basic_auth(&self.user, Some(&self.password))
            .json(&RpcRequest {
                jsonrpc: String::from("1.0"),
                id: self.get_req_id().await.to_string(),
                method,
                params,
            })
            .send()
            .await?;
        let resp: RpcServerMessage = resp.json().await?;
        match resp.body {
            RpcServerMessageBody::Notification { method: _, params } => {
                Ok(serde_json::from_value(params)?)
            }
            RpcServerMessageBody::Response { id: _, result } => {
                trace!("{}", result);
                Ok(serde_json::from_value(result)?)
            }
            RpcServerMessageBody::Error { id: _, error } => Err(CallError::RpcError(error)),
        }
    }

    pub(super) async fn estimatesmartfee(
        &self,
        conf_target: u32,
    ) -> Result<EstimateSmartFeeResponse, CallError> {
        Ok(
            match self
                .call(
                    "estimatesmartfee",
                    Value::Array(vec![Value::Number(conf_target.clamp(1, 1008).into())]),
                )
                .await
            {
                Ok(v) => v,
                Err(e) => return Err(e),
            },
        )
    }

    async fn get_req_id(&self) -> u64 {
        let mut counter = self.counter.lock().await;
        *counter += 1;
        *counter
    }

    async fn getbestblockhash(&self) -> Result<GetBestBlockHashResponse, CallError> {
        Ok(
            match self
                .call("getbestblockhash", Value::Array(Vec::new()))
                .await
            {
                Ok(v) => v,
                Err(e) => return Err(e),
            },
        )
    }

    async fn getblock(&self, block_hash: String) -> Result<GetBlockResponse, CallError> {
        Ok(
            match self
                .call(
                    "getblock",
                    Value::Array(vec![Value::String(block_hash), Value::Number(0.into())]),
                )
                .await
            {
                Ok(v) => v,
                Err(e) => return Err(e),
            },
        )
    }

    async fn getblockcount(&self) -> Result<GetBlockCountResponse, CallError> {
        Ok(
            match self.call("getblockcount", Value::Array(Vec::new())).await {
                Ok(v) => v,
                Err(e) => return Err(e),
            },
        )
    }

    async fn getblockheader(
        &self,
        block_hash: String,
    ) -> Result<GetBlockHeaderResponse, CallError> {
        Ok(
            match self
                .call(
                    "getblockheader",
                    Value::Array(vec![Value::String(block_hash)]),
                )
                .await
            {
                Ok(v) => v,
                Err(e) => return Err(e),
            },
        )
    }

    async fn getrawtransaction(
        &self,
        txid: String,
    ) -> Result<GetRawTransactionResponse, CallError> {
        Ok(
            match self
                .call("getrawtransaction", Value::Array(vec![Value::String(txid)]))
                .await
            {
                Ok(v) => v,
                Err(e) => return Err(e),
            },
        )
    }

    async fn sendrawtransaction(
        &self,
        hex: String,
    ) -> Result<SendRawTransactionResponse, CallError> {
        Ok(
            match self
                .call("sendrawtransaction", Value::Array(vec![Value::String(hex)]))
                .await
            {
                Ok(v) => v,
                Err(e) => return Err(e),
            },
        )
    }
}

#[async_trait::async_trait]
impl ChainClient for BitcoindClient {
    async fn broadcast_tx(&self, tx: Transaction) -> Result<(), BroadcastError> {
        let hex = serialize_hex(&tx);
        trace!(tx = hex, "broadcasting tx");
        self.sendrawtransaction(hex).await?;
        Ok(())
    }

    async fn get_blockheight(&self) -> Result<u64, ChainError> {
        Ok(self.getblockcount().await?.n)
    }

    async fn get_tip_hash(&self) -> Result<BlockHash, ChainError> {
        let hex = self.getbestblockhash().await?.hex;
        Ok(BlockHash::from_raw_hash(sha256d::Hash::from_str(&hex)?))
    }

    async fn get_block(&self, hash: &BlockHash) -> Result<Block, ChainError> {
        let hex = self.getblock(hash.to_string()).await?.hex;
        let raw: Vec<u8> = FromHex::from_hex(&hex)?;
        Ok(deserialize(&raw)?)
    }

    async fn get_block_header(&self, hash: &BlockHash) -> Result<BlockHeader, ChainError> {
        let resp = self.getblockheader(hash.to_string()).await?;
        Ok(BlockHeader {
            hash: resp.hash.parse()?,
            height: resp.height,
            prev: resp.previousblockhash.parse()?,
        })
    }

    async fn get_sender_addresses(&self, utxos: &[OutPoint]) -> Result<Vec<Address>, ChainError> {
        let mut addresses = Vec::new();
        for utxo in utxos {
            let tx = self.getrawtransaction(utxo.txid.to_string()).await?;
            let tx = hex::decode(tx.str)
                .map_err(|e| ChainError::General(format!("invalid tx hex: {:?}", e).into()))?;
            let tx = Transaction::consensus_decode(&mut &tx[..])?;
            for vin in tx.input {
                let txin = self
                    .getrawtransaction(vin.previous_output.txid.to_string())
                    .await?;
                let txin = hex::decode(txin.str)
                    .map_err(|e| ChainError::General(format!("invalid tx hex: {:?}", e).into()))?;
                let txin = Transaction::consensus_decode(&mut &txin[..])?;
                if txin.output.len() < vin.previous_output.vout as usize {
                    return Err(ChainError::General("txin output does not exist".into()));
                }
                let txout = &txin.output[vin.previous_output.vout as usize];
                let address = Address::from_script(&txout.script_pubkey, self.network)?;
                addresses.push(address);
            }
        }

        Ok(addresses)
    }
}

impl From<bitcoin::consensus::encode::Error> for ChainError {
    fn from(value: bitcoin::consensus::encode::Error) -> Self {
        ChainError::General(Box::new(value))
    }
}

impl From<CallError> for ChainError {
    fn from(value: CallError) -> Self {
        match value {
            CallError::RpcError(e) => ChainError::General(e.message.into()),
            CallError::Deserialize(e) => ChainError::General(Box::new(e)),
            CallError::General(e) => ChainError::General(e),
        }
    }
}

impl From<CallError> for BroadcastError {
    fn from(value: CallError) -> Self {
        match value {
            CallError::RpcError(rpc_error) => match &rpc_error.message {
                x if x.contains("insufficient fee, rejecting replacement") => {
                    BroadcastError::InsufficientFeeRejectingReplacement(rpc_error.message)
                }
                _ => BroadcastError::UnknownError(rpc_error.message),
            },
            CallError::Deserialize(_) => BroadcastError::Chain(value.into()),
            CallError::General(_) => BroadcastError::Chain(value.into()),
        }
    }
}

impl From<serde_json::error::Error> for CallError {
    fn from(value: serde_json::error::Error) -> Self {
        CallError::Deserialize(value)
    }
}

impl From<bitcoin::address::FromScriptError> for ChainError {
    fn from(value: bitcoin::address::FromScriptError) -> Self {
        ChainError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::HexToArrayError> for ChainError {
    fn from(value: bitcoin::hashes::hex::HexToArrayError) -> Self {
        ChainError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::HexToBytesError> for ChainError {
    fn from(value: bitcoin::hashes::hex::HexToBytesError) -> Self {
        ChainError::General(Box::new(value))
    }
}
