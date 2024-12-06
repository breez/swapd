use bitcoin::{BlockHash, OutPoint, TxOut};

#[derive(Clone, Debug)]
pub struct Utxo {
    pub block_hash: BlockHash,
    pub block_height: u64,
    pub outpoint: OutPoint,
    pub tx_out: TxOut,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockHeader {
    pub hash: BlockHash,
    pub height: u64,
    pub prev: BlockHash,
}
