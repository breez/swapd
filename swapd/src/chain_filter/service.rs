use std::sync::Arc;

use futures::{stream::FuturesUnordered, StreamExt};
use tracing::{debug, field};

use crate::chain::{ChainClient, Txo};

use super::ChainFilterRepository;

#[async_trait::async_trait]
pub trait ChainFilterService {
    async fn filter_txos(&self, utxos: Vec<Txo>) -> Result<Vec<Txo>, Box<dyn std::error::Error>>;
}

#[derive(Debug)]
pub struct ChainFilterImpl<C, R>
where
    C: ChainClient,
    R: ChainFilterRepository,
{
    chain_client: Arc<C>,
    repository: Arc<R>,
}

impl<C, R> ChainFilterImpl<C, R>
where
    C: ChainClient,
    R: ChainFilterRepository,
{
    pub fn new(chain_client: Arc<C>, repository: Arc<R>) -> Self {
        Self {
            chain_client,
            repository,
        }
    }

    async fn should_filter_txo(&self, txo: Txo) -> Result<bool, Box<dyn std::error::Error>> {
        let sender_addresses = self
            .chain_client
            .get_sender_addresses(&[txo.outpoint])
            .await?;
        self.repository
            .has_filtered_address(&sender_addresses)
            .await
    }
}

#[async_trait::async_trait]
impl<C, R> ChainFilterService for ChainFilterImpl<C, R>
where
    C: ChainClient + Send + Sync,
    R: ChainFilterRepository + Send + Sync,
{
    async fn filter_txos(&self, txos: Vec<Txo>) -> Result<Vec<Txo>, Box<dyn std::error::Error>> {
        let mut futures = FuturesUnordered::new();
        for txo in txos {
            let fut = self.should_filter_txo(txo.clone());
            futures.push(async {
                let should_filter_res = fut.await;
                (txo, should_filter_res)
            })
        }

        let mut result = Vec::new();
        while let Some((txo, should_filter_res)) = futures.next().await {
            if should_filter_res? {
                debug!(outpoint = field::display(txo.outpoint), "filtering utxo");
                continue;
            }
            result.push(txo);
        }
        Ok(result)
    }
}
