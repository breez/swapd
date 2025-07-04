use bitcoin::{BlockHash, OutPoint, TxOut, Txid};

#[derive(Clone, Debug)]
pub struct Txo {
    pub block_hash: BlockHash,
    pub block_height: u64,
    pub outpoint: OutPoint,
    pub tx_out: TxOut,
}

impl Txo {
    pub fn confirmations(&self, current_height: u64) -> u64 {
        current_height
            .saturating_add(1)
            .saturating_sub(self.block_height)
    }
}

#[derive(Clone, Debug)]
pub struct TxoWithSpend {
    pub txo: Txo,
    pub spend: Option<TxoSpend>,
}

#[derive(Clone, Debug)]
pub struct TxoSpend {
    pub spending_tx: Txid,
    pub spending_input_index: u32,
    pub block_hash: BlockHash,
    pub block_height: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockHeader {
    pub hash: BlockHash,
    pub height: u64,
    pub prev: BlockHash,
}
