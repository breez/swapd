use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error;

pub trait RandomProvider {
    fn rnd_32(&self) -> Result<[u8; 32], RandomError>;
}

#[derive(Debug, Error)]
pub enum RandomError {
    #[error("general: {0}")]
    General(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub struct RingRandomProvider {
    rnd: SystemRandom,
}

impl RingRandomProvider {
    pub fn new() -> Self {
        Self {
            rnd: SystemRandom::new(),
        }
    }
}

impl RandomProvider for RingRandomProvider {
    fn rnd_32(&self) -> Result<[u8; 32], RandomError> {
        let mut val = [0u8; 32];
        self.rnd.fill(&mut val)?;
        Ok(val)
    }
}

impl From<ring::error::Unspecified> for RandomError {
    fn from(_value: ring::error::Unspecified) -> Self {
        RandomError::General("unspecified error".into())
    }
}
