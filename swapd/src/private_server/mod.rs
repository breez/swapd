pub mod internal_swap_api {
    tonic::include_proto!("swap_internal");
}

mod server;

pub use server::Server;