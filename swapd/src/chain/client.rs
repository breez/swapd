use bitcoin::OutPoint;

#[derive(Debug)]
pub struct Utxo {
    pub block_height: u32,
    pub outpoint: OutPoint,
    pub amount_sat: u64,
}

#[derive(Debug)]
pub enum ChainError {
    General(Box<dyn std::error::Error>),
}

#[async_trait::async_trait]
pub trait ChainClient {
    async fn get_blockheight(&self) -> Result<u32, ChainError>;
}
