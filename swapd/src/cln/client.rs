use crate::{
    chain::{ChainClient, ChainError},
    lightning::{LightningClient, PayError},
};

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl LightningClient for Client {
    async fn pay(&self, _bolt11: String) -> Result<[u8; 32], PayError> {
        todo!()
    }
}

#[async_trait::async_trait]
impl ChainClient for Client {
    async fn get_blockheight(&self) -> Result<u32, ChainError> {
        todo!()
    }
}
