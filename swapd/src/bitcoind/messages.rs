use serde::Deserialize;

#[derive(Deserialize)]
pub struct EstimateSmartFeeResponse {
    pub feerate: f64,
}

pub struct GetBestBlockHashResponse {
    pub hex: String,
}

impl<'de> Deserialize<'de> for GetBestBlockHashResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(GetBestBlockHashResponse {
            hex: Deserialize::deserialize(deserializer)?,
        })
    }
}

#[derive(Deserialize)]
pub struct GetBlockHeaderResponse {
    pub hash: String,
    pub height: u64,
    pub previousblockhash: String,
}

pub struct GetBlockResponse {
    pub hex: String,
}

impl<'de> Deserialize<'de> for GetBlockResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(GetBlockResponse {
            hex: Deserialize::deserialize(deserializer)?,
        })
    }
}

pub struct GetBlockCountResponse {
    pub n: u64,
}

impl<'de> Deserialize<'de> for GetBlockCountResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(GetBlockCountResponse {
            n: Deserialize::deserialize(deserializer)?,
        })
    }
}

pub struct GetRawTransactionResponse {
    pub str: String,
}

impl<'de> Deserialize<'de> for GetRawTransactionResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(GetRawTransactionResponse {
            str: Deserialize::deserialize(deserializer)?,
        })
    }
}

pub struct SendRawTransactionResponse {
    #[allow(unused)]
    pub hex: String,
}

impl<'de> Deserialize<'de> for SendRawTransactionResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(SendRawTransactionResponse {
            hex: Deserialize::deserialize(deserializer)?,
        })
    }
}
