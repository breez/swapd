use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("{0}")]
    General(Box<dyn std::error::Error>),
}

#[async_trait::async_trait]
pub trait Repository {
    async fn add_label(&self, label: String, payment_index: u64) -> Result<(), RepositoryError>;
    async fn get_label(&self, payment_index: u64) -> Result<Option<String>, RepositoryError>;
    async fn get_payment_index(&self, label: &str) -> Result<Option<u64>, RepositoryError>;
}
