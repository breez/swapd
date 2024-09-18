mod client;
mod wallet;

pub use client::{Client, ClientConnection};

pub mod cln_api {
    tonic::include_proto!("cln");
}
