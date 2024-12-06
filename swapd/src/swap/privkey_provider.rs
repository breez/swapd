use bitcoin::secp256k1::SecretKey;
use ring::rand::{SecureRandom, SystemRandom};

pub trait PrivateKeyProvider {
    fn new_private_key(&self) -> Result<SecretKey, Box<dyn std::error::Error>>;
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
    fn new_private_key(&self) -> Result<SecretKey, Box<dyn std::error::Error>> {
        let mut key = [0u8; 32];
        self.rnd
            .fill(&mut key)
            .map_err(|_| "failed to generate key")?;
        Ok(SecretKey::from_slice(&key)?)
    }
}
