mod client;
mod wallet;

pub use client::{Client, ClientConnection};

pub mod cln_api {
    #![allow(clippy::all)]
    #![allow(dead_code)]
    tonic::include_proto!("cln");
}
