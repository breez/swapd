mod client;
mod wallet;

pub use client::{Client, ClientConnection};

pub mod cln_api {
    #![allow(clippy::all)]
    tonic::include_proto!("cln");
}
