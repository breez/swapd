use std::sync::Arc;

use futures::{stream::FuturesUnordered, StreamExt};

use crate::chain::{ChainClient, Utxo};

use super::ChainFilterRepository;

#[async_trait::async_trait]
pub trait ChainFilterService {
    async fn filter_utxos(&self, utxos: Vec<Utxo>)
        -> Result<Vec<Utxo>, Box<dyn std::error::Error>>;
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

    async fn should_filter_utxo(&self, utxo: Utxo) -> Result<bool, Box<dyn std::error::Error>> {
        let sender_addresses = self
            .chain_client
            .get_sender_addresses(&[utxo.outpoint])
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
    async fn filter_utxos(
        &self,
        utxos: Vec<Utxo>,
    ) -> Result<Vec<Utxo>, Box<dyn std::error::Error>> {
        let mut futures = FuturesUnordered::new();
        for utxo in utxos {
            let fut = self.should_filter_utxo(utxo.clone());
            futures.push(async {
                let should_filter_res = fut.await;
                (utxo, should_filter_res)
            })
        }

        let mut result = Vec::new();
        while let Some((utxo, should_filter_res)) = futures.next().await {
            if should_filter_res? {
                continue;
            }
            result.push(utxo);
        }
        Ok(result)
    }
}
