use std::{
    sync::Arc,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use reqwest::Url;
use serde::Deserialize;
use thiserror::Error;
use tokio::{sync::Mutex, time::MissedTickBehavior};
use tracing::error;

use crate::chain::{FeeEstimate, FeeEstimateError, FeeEstimator};

const STALE_SECONDS: u64 = 60 * 12;

#[derive(Debug, Deserialize)]
struct WhatTheFeeResponse {
    index: Vec<i32>,
    columns: Vec<String>,
    data: Vec<Vec<i32>>,
}

#[derive(Debug)]
struct LastResponse {
    timestamp: SystemTime,
    response: WhatTheFeeResponse,
}

#[derive(Debug)]
pub struct WhatTheFeeEstimator {
    url: Url,
    lock_time: u32,
    last_response: Arc<Mutex<Option<LastResponse>>>,
}

#[derive(Debug, Error)]
pub enum WhatTheFeeError {
    #[error("whatthefee: {0}")]
    General(Box<dyn std::error::Error>),
}

impl WhatTheFeeEstimator {
    pub fn new(url: Url, lock_time: u32) -> Self {
        Self {
            url,
            lock_time,
            last_response: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self) -> Result<(), WhatTheFeeError> {
        let fees = get_fees(&self.url).await?;
        *self.last_response.lock().await = Some(fees);
        self.run_forever();
        Ok(())
    }

    fn run_forever(&self) {
        let last_response = Arc::clone(&self.last_response);
        let url = self.url.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let fees = match get_fees(&url).await {
                    Ok(fees) => fees,
                    Err(e) => {
                        error!("failed to get fees: {:?}", e);
                        continue;
                    }
                };
                *last_response.lock().await = Some(fees);
            }
        });
    }
}

#[async_trait::async_trait]
impl FeeEstimator for WhatTheFeeEstimator {
    async fn estimate_fee(&self, conf_target: i32) -> Result<FeeEstimate, FeeEstimateError> {
        let last_response = &*self.last_response.lock().await;
        let last_response = match last_response {
            Some(last_response) => last_response,
            None => return Err(FeeEstimateError::Unavailable),
        };

        let now = SystemTime::now();
        let response_age = match now.duration_since(last_response.timestamp) {
            Ok(response_age) => response_age,
            Err(_) => return Err(FeeEstimateError::Unavailable),
        };

        if response_age.as_secs() > STALE_SECONDS {
            return Err(FeeEstimateError::Unavailable);
        }

        let last_response = &last_response.response;
        if last_response.index.is_empty() {
            return Err(FeeEstimateError::Unavailable);
        }

        let mut row_index = 0;
        let mut prev_row = last_response.index[row_index];
        for (i, current_row) in last_response.index.iter().skip(1).enumerate() {
            if (current_row - conf_target).abs() < (prev_row - conf_target).abs() {
                row_index = i + 1;
                prev_row = *current_row;
            }
        }

        if last_response.columns.is_empty() {
            return Err(FeeEstimateError::Unavailable);
        }

        let certainty =
            0.5 + (((self.lock_time as f64 - conf_target as f64) / self.lock_time as f64) / 2.);
        let mut column_index = 0;
        let mut prev_column: f64 = match last_response.columns[column_index].parse() {
            Ok(prev_column) => prev_column,
            Err(_) => return Err(FeeEstimateError::Unavailable),
        };
        for (i, current_column) in last_response.columns.iter().skip(1).enumerate() {
            let current_column: f64 = match current_column.parse() {
                Ok(current_column) => current_column,
                Err(_) => return Err(FeeEstimateError::Unavailable),
            };

            if (current_column - certainty).abs() < (prev_column - certainty).abs() {
                column_index = i + 1;
                prev_column = current_column;
            }
        }

        if row_index >= last_response.data.len() {
            return Err(FeeEstimateError::Unavailable);
        }
        let row = &last_response.data[row_index];
        if column_index >= row.len() {
            return Err(FeeEstimateError::Unavailable);
        }
        let rate = row[column_index] as f64;
        let sat_per_vbyte = (rate / 100.).exp();
        let sat_per_kw = (sat_per_vbyte * 250.) as u32;
        Ok(FeeEstimate { sat_per_kw })
    }
}

async fn get_fees(url: &Url) -> Result<LastResponse, WhatTheFeeError> {
    let now = SystemTime::now();
    let timestamp = now.duration_since(UNIX_EPOCH)?.as_secs();
    let cache_bust = (timestamp / 300) * 300;
    let mut url = url.clone();
    url.query_pairs_mut()
        .append_pair("c", &cache_bust.to_string());
    let response = reqwest::get(url)
        .await?
        .json::<WhatTheFeeResponse>()
        .await?;
    Ok(LastResponse {
        response,
        timestamp: now,
    })
}

impl From<reqwest::Error> for WhatTheFeeError {
    fn from(value: reqwest::Error) -> Self {
        WhatTheFeeError::General(Box::new(value))
    }
}

impl From<SystemTimeError> for WhatTheFeeError {
    fn from(value: SystemTimeError) -> Self {
        WhatTheFeeError::General(Box::new(value))
    }
}
