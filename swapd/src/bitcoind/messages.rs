use serde::Deserialize;

#[derive(Deserialize)]
pub struct GetBlockCountResponse {
    pub n: u32,
}

#[derive(Deserialize)]
pub struct GetRawTransactionResponse {
    pub str: String,
}
