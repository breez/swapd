use bitcoin::secp256k1::SecretKey;
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error;

pub trait PrivateKeyProvider {
    fn new_private_key(&self) -> Result<SecretKey, PrivateKeyError>;
}

#[derive(Debug, Error)]
pub enum PrivateKeyError {
    #[error("general: {0}")]
    General(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub struct RandomPrivateKeyProvider {
    rnd: SystemRandom,
}

impl RandomPrivateKeyProvider {
    pub fn new() -> Self {
        Self {
            rnd: SystemRandom::new(),
        }
    }
}

impl PrivateKeyProvider for RandomPrivateKeyProvider {
    fn new_private_key(&self) -> Result<SecretKey, PrivateKeyError> {
        let mut key = [0u8; 32];
        self.rnd.fill(&mut key)?;
        Ok(SecretKey::from_slice(&key)?)
    }
}

impl From<ring::error::Unspecified> for PrivateKeyError {
    fn from(_value: ring::error::Unspecified) -> Self {
        PrivateKeyError::General("unspecified error".into())
    }
}

impl From<bitcoin::secp256k1::Error> for PrivateKeyError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        PrivateKeyError::General(Box::new(value))
    }
}
