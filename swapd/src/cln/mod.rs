mod client;
mod wallet;

pub use client::Client;

pub mod cln_api {
    tonic::include_proto!("cln");
}
