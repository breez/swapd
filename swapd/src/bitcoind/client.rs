use bitcoin::{Address, OutPoint};

use crate::chain::{ChainClient, ChainError};

#[derive(Debug)]
pub struct BitcoindClient {
    address: String,
    user: String,
    password: String,
}

impl BitcoindClient {
    pub fn new(address: String, user: String, password: String) -> Self {
        Self {
            address,
            user,
            password
        }
    }
}


#[async_trait::async_trait]
impl ChainClient for BitcoindClient {
    async fn get_blockheight(&self) -> Result<u32, ChainError> {
        todo!()
    }

    async fn get_sender_addresses(&self, utxos: &[OutPoint]) -> Result<Vec<Address>, ChainError> {
        todo!()
    }
}