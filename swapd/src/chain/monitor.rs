use std::{collections::HashMap, future::Future, pin::pin, sync::Arc, time::Duration};

use bitcoin::{block::Bip34Error, Address, Block, BlockHash, Network, OutPoint};
use futures::future::{FusedFuture, FutureExt};
use tracing::debug;

use crate::chain::{AddressUtxo, ChainClient, ChainRepository, SpentUtxo, Utxo};

use super::{types::BlockHeader, ChainError, ChainRepositoryError};

pub struct ChainMonitor<C, R>
where
    C: ChainClient,
    R: ChainRepository,
{
    network: Network,
    chain_client: Arc<C>,
    chain_repository: Arc<R>,
    poll_interval: Duration,
}

impl<C, R> ChainMonitor<C, R>
where
    C: ChainClient,
    R: ChainRepository,
{
    pub fn new(
        network: Network,
        chain_client: Arc<C>,
        chain_repository: Arc<R>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            chain_client,
            network,
            chain_repository,
            poll_interval,
        }
    }

    pub async fn start<F: Future<Output = ()>>(&self, signal: F) -> Result<(), ChainError> {
        let blocks = self.chain_repository.get_block_headers().await?;
        let chain = match Chain::try_from(blocks) {
            Ok(chain) => chain,
            Err(e) => match e {
                ChainError::EmptyChain => {
                    // If the chain is empty, set the birthday to 20 blocks ago.
                    let tip_hash = self.chain_client.get_tip_hash().await?;
                    let mut birthday_header = self.chain_client.get_block_header(&tip_hash).await?;
                    for _n in 1..21 {
                        birthday_header = self
                            .chain_client
                            .get_block_header(&birthday_header.prev)
                            .await?;
                    }

                    self.chain_repository.add_block(&birthday_header).await?;
                    let mut chain = Chain::new(birthday_header.hash);
                    chain.blocks.insert(
                        birthday_header.hash,
                        BlockInfo {
                            hash: birthday_header.hash,
                            prev: birthday_header.prev,
                            next: None,
                        },
                    );
                    chain
                }
                _ => return Err(e),
            },
        };

        let mut sig = pin!(signal.fuse());
        if sig.is_terminated() {
            return Ok(());
        }

        loop {
            self.do_sync(&chain).await?;

            tokio::select! {
                _ = &mut sig => {
                    debug!("chain monitor shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {}
            }
        }

        Ok(())
    }

    async fn do_sync(&self, existing_chain: &Chain) -> Result<Chain, ChainError>
    where
        C: ChainClient,
        R: ChainRepository,
    {
        let tip_hash = self.chain_client.get_tip_hash().await?;
        let mut new_chain = Chain::new(tip_hash);
        let mut current_hash = tip_hash;
        let mut next_hash = None;

        // Iterate backwards from the tip to get the missed block headers.
        loop {
            // Note that this is not checking the existing tip, because the chain
            // may have reorged.
            if existing_chain.blocks.contains_key(&current_hash) {
                break;
            }

            let current_header = self.chain_client.get_block_header(&current_hash).await?;
            new_chain.blocks.insert(
                tip_hash,
                BlockInfo {
                    hash: current_hash,
                    prev: current_header.prev,
                    next: next_hash,
                },
            );
            current_hash = current_header.prev;
            next_hash = Some(current_hash);
        }

        // If the base and tip don't match, there was a reorg.
        if new_chain.base != existing_chain.tip {
            let mut reorg_block_hash = existing_chain.tip;
            loop {
                if new_chain.blocks.contains_key(&reorg_block_hash) {
                    break;
                }

                self.chain_repository.undo_block(reorg_block_hash).await?;
                let reorg_block = existing_chain
                    .blocks
                    .get(&reorg_block_hash)
                    .expect("in-memory chain does not contain expected block!");
                reorg_block_hash = reorg_block.prev;
            }
        }

        // Iterate forward from the last known block to the tip to process blocks.
        // Note that this always re-processes the last known block.
        let mut current_block = &new_chain.blocks[&new_chain.base];
        loop {
            let block = self.chain_client.get_block(&current_block.hash).await?;
            self.process_block(&block).await?;
            match current_block.next {
                Some(next) => current_block = &new_chain.blocks[&next],
                None => break,
            }
        }

        new_chain.rebase(existing_chain);

        Ok(new_chain)
    }

    async fn process_block(&self, block: &Block) -> Result<(), ChainError> {
        // Check all transactions in the block
        // - does an output send to a known address?
        // - does an input spend a known utxo?
        let block_hash = block.block_hash();
        let prev_block_hash = block.header.prev_blockhash;
        let block_height = block.bip34_block_height()?;
        let mut spent_utxos = Vec::new();
        let mut addresses = Vec::new();
        let mut address_utxos = HashMap::new();
        for tx in &block.txdata {
            let txid = tx.txid();
            for (vout, output) in tx.output.iter().enumerate() {
                let address = Address::from_script(&output.script_pubkey, self.network)?;
                addresses.push(address.clone());
                let entry = address_utxos.entry(address).or_insert(Vec::new());
                entry.push((OutPoint::new(txid, vout as u32), output.value));
            }

            for input in &tx.input {
                spent_utxos.push(SpentUtxo {
                    spending_block: block_hash,
                    spending_tx: txid,
                    outpoint: input.previous_output,
                });
            }
        }

        let watch_addresses = self
            .chain_repository
            .filter_watch_addresses(&addresses)
            .await?;
        let watch_utxos: Vec<AddressUtxo> = watch_addresses
            .into_iter()
            .filter_map(|a| {
                address_utxos.get(&a).map(|out| {
                    out.iter()
                        .map(|(outpoint, amount_sat)| AddressUtxo {
                            address: a.clone(),
                            utxo: Utxo {
                                amount_sat: *amount_sat,
                                block_hash,
                                block_height,
                                outpoint: *outpoint,
                            },
                        })
                        .collect()
                })
            })
            .flat_map(|a: Vec<AddressUtxo>| a)
            .collect();
        self.chain_repository.add_utxos(&watch_utxos).await?;
        self.chain_repository.mark_spent(&spent_utxos).await?;
        self.chain_repository
            .add_block(&BlockHeader {
                hash: block_hash,
                height: block_height,
                prev: prev_block_hash,
            })
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
struct Chain {
    tip: BlockHash,
    base: BlockHash,
    blocks: HashMap<BlockHash, BlockInfo>,
}

#[derive(Clone)]
struct BlockInfo {
    hash: BlockHash,
    prev: BlockHash,
    next: Option<BlockHash>,
}

impl Chain {
    fn new(tip: BlockHash) -> Self {
        Chain {
            tip,
            base: tip,
            blocks: HashMap::new(),
        }
    }

    fn rebase(&mut self, other: &Chain) {
        let mut next_block = self.blocks[&self.base].clone();
        loop {
            if next_block.hash == other.base {
                break;
            }

            let mut current_block = other.blocks[&next_block.prev].clone();
            current_block.next = Some(next_block.hash);
            self.base = current_block.hash;
            self.blocks
                .insert(current_block.hash, current_block.clone());
            next_block = current_block;
        }
    }
}

impl TryFrom<Vec<super::types::BlockHeader>> for Chain {
    type Error = ChainError;

    fn try_from(headers: Vec<super::types::BlockHeader>) -> Result<Self, Self::Error> {
        if headers.is_empty() {
            return Err(ChainError::EmptyChain);
        }

        let tip_header = &headers[0];
        let mut chain = Chain::new(tip_header.hash);
        chain.blocks.insert(
            tip_header.hash,
            BlockInfo {
                hash: tip_header.hash,
                prev: tip_header.prev,
                next: None,
            },
        );
        let mut next = tip_header;
        for header in headers.iter().skip(1) {
            if header.hash != next.prev {
                return Err(ChainError::InvalidChain);
            }
            chain.blocks.insert(
                header.hash,
                BlockInfo {
                    hash: header.hash,
                    prev: header.prev,
                    next: Some(next.hash),
                },
            );
            chain.base = header.hash;
            next = header;
        }
        Ok(chain)
    }
}

impl From<ChainRepositoryError> for ChainError {
    fn from(value: ChainRepositoryError) -> Self {
        ChainError::Database(value)
    }
}

impl From<Bip34Error> for ChainError {
    fn from(value: Bip34Error) -> Self {
        ChainError::General(Box::new(value))
    }
}
