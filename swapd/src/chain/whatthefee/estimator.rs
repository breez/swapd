use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use tokio::{sync::Mutex, time::MissedTickBehavior};

use crate::chain::{FeeEstimate, FeeEstimateError, FeeEstimator};

const STALE_SECONDS: u64 = 60 * 12;

#[derive(Deserialize)]
struct WhatTheFeeResponse {
    index: Vec<i32>,
    columns: Vec<String>,
    data: Vec<Vec<i32>>,
}

struct LastResponse {
    timestamp: SystemTime,
    response: WhatTheFeeResponse,
}

pub struct WhatTheFeeEstimator {
    lock_time: u32,
    last_response: Arc<Mutex<Option<LastResponse>>>,
}

impl WhatTheFeeEstimator {
    pub fn new(lock_time: u32) -> Self {
        Self {
            lock_time,
            last_response: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let fees = get_fees().await?;
        *self.last_response.lock().await = Some(fees);
        self.run_forever();
        Ok(())
    }

    fn run_forever(&self) {
        let last_response = Arc::clone(&self.last_response);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let fees = match get_fees().await {
                    Ok(fees) => fees,
                    Err(e) => break,
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
            Err(e) => return Err(FeeEstimateError::Unavailable),
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
            Err(e) => return Err(FeeEstimateError::Unavailable),
        };
        for (i, current_column) in last_response.columns.iter().skip(1).enumerate() {
            let current_column: f64 = match current_column.parse() {
                Ok(current_column) => current_column,
                Err(e) => return Err(FeeEstimateError::Unavailable),
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

async fn get_fees() -> Result<LastResponse, Box<dyn std::error::Error>> {
    let now = SystemTime::now();
    let timestamp = now.duration_since(UNIX_EPOCH)?.as_secs();
    let cache_bust = (timestamp / 300) * 300;
    let response = reqwest::get(format!("https://whatthefee.io/data.json?c={}", cache_bust))
        .await?
        .json::<WhatTheFeeResponse>()
        .await?;
    Ok(LastResponse {
        response,
        timestamp: now,
    })
}
