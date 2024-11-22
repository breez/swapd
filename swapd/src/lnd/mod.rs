mod client;
mod repository;
mod wallet;

pub use client::{Client, ClientConnection};
pub use repository::{Repository, RepositoryError};

pub mod lnrpc {
    #![allow(clippy::all)]
    tonic::include_proto!("lnrpc");
}

pub mod routerrpc {
    #![allow(clippy::all)]
    tonic::include_proto!("routerrpc");
}
