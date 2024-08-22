use bitcoin::{Network, PrivateKey};
use ring::rand::{SecureRandom, SystemRandom};

pub trait PrivateKeyProvider {
    fn new_private_key(&self) -> Result<PrivateKey, Box<dyn std::error::Error>>;
}

#[derive(Debug)]
pub struct RandomPrivateKeyProvider {
    rnd: SystemRandom,
    network: Network,
}

impl RandomPrivateKeyProvider {
    pub fn new(network: impl Into<Network>) -> Self {
        Self {
            rnd: SystemRandom::new(),
            network: network.into(),
        }
    }
}

impl PrivateKeyProvider for RandomPrivateKeyProvider {
    fn new_private_key(&self) -> Result<PrivateKey, Box<dyn std::error::Error>> {
        let mut key = [0u8; 32];
        self.rnd
            .fill(&mut key)
            .map_err(|_| "failed to generate key")?;
        Ok(PrivateKey::from_slice(&key, self.network)?)
    }
}
