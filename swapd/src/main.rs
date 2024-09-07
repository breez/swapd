use std::{sync::Arc, time::Duration};

use bitcoin::Network;
use bitcoind::BitcoindClient;
use chain::ChainMonitor;
use chain_filter::ChainFilterImpl;
use clap::Parser;
use internal_server::internal_swap_api::swap_manager_server::SwapManagerServer;
use public_server::{swap_api::swapper_server::SwapperServer, SwapServer, SwapServerParams};
use sqlx::PgPool;
use swap::{RandomPrivateKeyProvider, SwapService};
use tokio::signal;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tonic::transport::{Server, Uri};
use tracing::{field, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use whatthefee::WhatTheFeeEstimator;

mod bitcoind;
mod chain;
mod chain_filter;
mod cln;
mod internal_server;
mod lightning;
mod postgresql;
mod public_server;
mod redeem;
mod swap;
mod whatthefee;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Address the grpc server will listen on.
    #[arg(long)]
    pub address: core::net::SocketAddr,

    /// Address the internal grpc server will listen on.
    #[arg(long)]
    pub internal_address: core::net::SocketAddr,

    /// Maximum amount allowed for swaps.
    #[arg(long, default_value = "4_000_000")]
    pub max_swap_amount_sat: u64,

    /// Locktime for swaps. This is the time between confirmation of the swap
    /// until the client can get a refund.
    #[arg(long, default_value = "288")]
    pub lock_time: u32,

    /// Minimum number of confirmations required before a swap is eligible for
    /// payout.
    #[arg(long, default_value = "1")]
    pub min_confirmations: u64,

    /// Minimum number of blocks needed to redeem a utxo onchain. A utxo is no
    /// longer eligible for payout when the lock time left is less than this
    /// number.
    #[arg(long, default_value = "72")]
    pub min_redeem_blocks: u32,

    /// Bitcoin network. Valid values are bitcoin, testnet, signet, regtest.
    #[arg(long, default_value = "bitcoin")]
    pub network: Network,

    /// Amount of satoshis below which an output is considered dust.
    #[arg(long, default_value = "546")]
    pub dust_limit_sat: u64,

    /// Address to the cln grpc api.
    #[arg(long)]
    pub cln_grpc_address: Uri,

    /// Loglevel to use. Can be used to filter loges through the env filter
    /// format.
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Connectionstring to the postgres database.
    #[arg(long)]
    pub db_url: String,

    /// Address to the bitcoind rpc.
    #[arg(long)]
    pub bitcoind_rpc_address: String,

    /// Bitcoind rpc username.
    #[arg(long)]
    pub bitcoind_rpc_user: String,

    /// Bitcoind rpc password.
    #[arg(long)]
    pub bitcoind_rpc_password: String,

    /// Polling interval between chain syncs.
    #[arg(long, default_value = "20")]
    pub chain_poll_interval_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    tracing_subscriber::registry()
        .with(EnvFilter::new(args.log_level))
        .init();

    let privkey_provider = RandomPrivateKeyProvider::new(args.network);
    let swap_service = Arc::new(SwapService::new(
        args.network,
        privkey_provider,
        args.lock_time,
        args.dust_limit_sat,
    ));

    let chain_client = Arc::new(BitcoindClient::new(
        args.bitcoind_rpc_address,
        args.bitcoind_rpc_user,
        args.bitcoind_rpc_password,
        args.network,
    ));
    let cln_client = Arc::new(cln::Client::new(args.cln_grpc_address));
    let pgpool = Arc::new(PgPool::connect(&args.db_url).await?);
    let swap_repository = Arc::new(postgresql::SwapRepository::new(
        Arc::clone(&pgpool),
        args.network,
    ));
    let chain_repository = Arc::new(postgresql::ChainRepository::new(
        Arc::clone(&pgpool),
        args.network,
    ));
    let chain_filter_repository =
        Arc::new(postgresql::ChainFilterRepository::new(Arc::clone(&pgpool)));
    let chain_filter = Arc::new(ChainFilterImpl::new(
        Arc::clone(&chain_client),
        Arc::clone(&chain_filter_repository),
    ));
    let fee_estimator = Arc::new(WhatTheFeeEstimator::new(args.lock_time));
    fee_estimator.start().await?;
    let swapper_server = SwapperServer::new(SwapServer::new(
        &SwapServerParams {
            network: args.network,
            max_swap_amount_sat: args.max_swap_amount_sat,
            min_confirmations: args.min_confirmations,
            min_redeem_blocks: args.min_redeem_blocks,
        },
        Arc::clone(&chain_client),
        Arc::clone(&chain_filter),
        Arc::clone(&chain_repository),
        Arc::clone(&cln_client),
        Arc::clone(&swap_service),
        Arc::clone(&swap_repository),
        Arc::clone(&fee_estimator),
    ));

    let internal_server = SwapManagerServer::new(internal_server::Server::new(
        args.network,
        chain_filter_repository,
    ));
    let chain_monitor = ChainMonitor::new(
        args.network,
        Arc::clone(&chain_client),
        Arc::clone(&chain_repository),
        Duration::from_secs(args.chain_poll_interval_seconds),
    );

    let token = CancellationToken::new();
    let server_token = token.clone();
    let internal_server_token = token.clone();
    let chain_monitor_token = token.clone();
    let tracker = TaskTracker::new();

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {}
            Err(err) => {
                warn!("Unable to listen for shutdown signal: {}", err);
            }
        }
        token.cancel();
    });
    tracker.spawn(async move {
        info!(
            address = field::display(&args.address),
            "Starting chain monitor"
        );
        let res = chain_monitor
            .start(async {
                chain_monitor_token.cancelled().await;
                info!("chain monitor shutting down");
            })
            .await;
        match res {
            Ok(_) => info!("chain monitor exited"),
            Err(e) => info!("chain monitor exited with {:?}", e),
        };
        chain_monitor_token.cancel();
    });
    tracker.spawn(async move {
        info!(
            address = field::display(&args.address),
            "Starting swapper server"
        );
        let res = Server::builder()
            .add_service(swapper_server)
            .serve_with_shutdown(args.address, async {
                server_token.cancelled().await;
                info!("swapper server shutting down");
            })
            .await;
        match res {
            Ok(_) => info!("swapper server exited"),
            Err(e) => info!("swapper server exited with {:?}", e),
        }

        server_token.cancel();
    });
    tracker.spawn(async move {
        info!(
            address = field::display(&args.internal_address),
            "Starting internal server"
        );
        let res = Server::builder()
            .add_service(internal_server)
            .serve_with_shutdown(args.internal_address, async {
                internal_server_token.cancelled().await;
                info!("internal server shutting down");
            })
            .await;
        match res {
            Ok(_) => info!("internal server exited"),
            Err(e) => info!("internal server exited with {:?}", e),
        }
        internal_server_token.cancel();
    });

    tracker.wait().await;
    info!("shutdown complete");
    Ok(())
}
