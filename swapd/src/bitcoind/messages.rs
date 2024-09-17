use serde::Deserialize;

#[derive(Deserialize)]
pub struct GetBestBlockHashResponse {
    pub hex: String,
}

#[derive(Deserialize)]
pub struct GetBlockHeaderResponse {
    pub hash: String,
    pub height: u64,
    pub previousblockhash: String,
}

#[derive(Deserialize)]
pub struct GetBlockResponse {
    pub hex: String,
}

#[derive(Deserialize)]
pub struct GetBlockCountResponse {
    pub n: u64,
}

#[derive(Deserialize)]
pub struct GetRawTransactionResponse {
    pub str: String,
}

#[derive(Deserialize)]
pub struct SendRawTransactionResponse {
    pub hex: String,
}
