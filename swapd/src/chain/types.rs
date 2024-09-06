use bitcoin::{BlockHash, OutPoint};

#[derive(Clone, Debug)]
pub struct Utxo {
    pub block_hash: BlockHash,
    pub block_height: u64,
    pub outpoint: OutPoint,
    pub amount_sat: u64,
}

#[derive(Clone, Debug)]
pub struct BlockHeader {
    pub hash: BlockHash,
    pub height: u64,
    pub prev: BlockHash,
}
