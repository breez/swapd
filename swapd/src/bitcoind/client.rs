use bitcoin::{consensus::Decodable, Address, Network, OutPoint, Transaction};
use reqwest::Method;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::chain::{ChainClient, ChainError};

use super::{
    GetBlockCountResponse, GetRawTransactionResponse, RpcError, RpcRequest, RpcServerMessage,
    RpcServerMessageBody,
};

#[derive(Debug)]
pub struct BitcoindClient {
    address: String,
    user: String,
    password: String,
    counter: Mutex<u64>,
    network: Network,
}

#[derive(Debug)]
enum CallError {
    RpcError(RpcError),
    Deserialize(serde_json::error::Error),
    General(Box<dyn std::error::Error>),
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
        let client = reqwest::Client::new();
        let resp = client
            .request(Method::POST, &self.address)
            .basic_auth(&self.user, Some(&self.password))
            .json(&RpcRequest {
                jsonrpc: String::from("1.0"),
                id: self.get_req_id().await.to_string(),
                method: method.into(),
                params,
            })
            .send()
            .await?;
        let resp: RpcServerMessage = resp.json().await?;
        match resp.body {
            RpcServerMessageBody::Notification { method: _, params } => {
                Ok(serde_json::from_value(params)?)
            }
            RpcServerMessageBody::Response { id: _, result } => Ok(serde_json::from_value(result)?),
            RpcServerMessageBody::Error { id: _, error } => Err(CallError::RpcError(error)),
        }
    }

    async fn get_req_id(&self) -> u64 {
        let mut counter = self.counter.lock().await;
        *counter += 1;
        *counter
    }

    async fn getblockcount(&self) -> Result<GetBlockCountResponse, CallError> {
        Ok(
            match self.call("getblockcount", Value::Array(Vec::new())).await {
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
}

#[async_trait::async_trait]
impl ChainClient for BitcoindClient {
    async fn get_blockheight(&self) -> Result<u32, ChainError> {
        Ok(self.getblockcount().await?.n)
    }

    async fn get_sender_addresses(&self, utxos: &[OutPoint]) -> Result<Vec<Address>, ChainError> {
        let mut addresses = Vec::new();
        for utxo in utxos {
            let tx = self.getrawtransaction(utxo.to_string()).await?;
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

impl From<serde_json::error::Error> for CallError {
    fn from(value: serde_json::error::Error) -> Self {
        CallError::Deserialize(value)
    }
}

impl From<bitcoin::address::Error> for ChainError {
    fn from(value: bitcoin::address::Error) -> Self {
        ChainError::General(Box::new(value))
    }
}
