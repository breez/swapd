use std::sync::Arc;

use bitcoin::secp256k1::SecretKey;
use thiserror::Error;

use super::random_provider::{RandomError, RandomProvider};

pub trait PrivateKeyProvider {
    fn new_private_key(&self) -> Result<SecretKey, PrivateKeyError>;
}

#[derive(Debug, Error)]
pub enum PrivateKeyError {
    #[error("general: {0}")]
    General(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub struct RandomPrivateKeyProvider<RP> {
    rnd: Arc<RP>,
}

impl<RP> RandomPrivateKeyProvider<RP>
where
    RP: RandomProvider,
{
    pub fn new(rnd: Arc<RP>) -> Self {
        Self { rnd }
    }
}

impl<RP> PrivateKeyProvider for RandomPrivateKeyProvider<RP>
where
    RP: RandomProvider,
{
    fn new_private_key(&self) -> Result<SecretKey, PrivateKeyError> {
        let key = self.rnd.rnd_32()?;
        Ok(SecretKey::from_slice(&key)?)
    }
}

impl From<RandomError> for PrivateKeyError {
    fn from(value: RandomError) -> Self {
        PrivateKeyError::General(format!("random error: {}", value).into())
    }
}

impl From<bitcoin::secp256k1::Error> for PrivateKeyError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        PrivateKeyError::General(Box::new(value))
    }
}
