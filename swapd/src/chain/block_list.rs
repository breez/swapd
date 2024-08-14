use super::Utxo;

#[async_trait::async_trait]
pub trait BlockListService {
    async fn filter_blocklisted(
        &self,
        utxos: &[Utxo],
    ) -> Result<Vec<Utxo>, Box<dyn std::error::Error>>;
}

#[derive(Debug)]
pub struct BlockListImpl {}

impl BlockListImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl BlockListService for BlockListImpl {
    async fn filter_blocklisted(
        &self,
        _utxos: &[Utxo],
    ) -> Result<Vec<Utxo>, Box<dyn std::error::Error>> {
        todo!()
    }
}
