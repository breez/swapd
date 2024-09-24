use std::{sync::Arc, time::Duration};

use bitcoin::Network;
use bitcoind::BitcoindClient;
use chain::{ChainMonitor, FallbackFeeEstimator};
use chain_filter::ChainFilterImpl;
use clap::Parser;
use internal_server::internal_swap_api::swap_manager_server::SwapManagerServer;
use public_server::{swap_api::swapper_server::SwapperServer, SwapServer, SwapServerParams};
use redeem::{PreimageMonitor, RedeemMonitor, RedeemMonitorParams, RedeemService};
use reqwest::Url;
use sqlx::PgPool;
use swap::{RandomPrivateKeyProvider, SwapService};
use tokio::signal;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tonic::transport::{Certificate, Identity, Server, Uri};
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
mod wallet;
mod whatthefee;

#[derive(Clone, Debug)]
struct FileOrCert(String);

impl From<std::string::String> for FileOrCert {
    fn from(value: std::string::String) -> Self {
        Self(value)
    }
}

impl FileOrCert {
    async fn resolve(&self) -> String {
        match tokio::fs::read_to_string(&self.0).await {
            Ok(content) => content,
            Err(_) => self.0.clone(),
        }
    }
}

#[derive(Clone, Parser, Debug)]
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

    /// Client key for grpc access. Can either be a file path or the key
    /// contents. Typically stored in `lightningd-dir/{network}/client-key.pem`.
    #[arg(long)]
    pub cln_grpc_ca_cert: FileOrCert,

    /// Client cert for grpc access. Can either be a file path or the cert
    /// contents. Typically stored in `lightningd-dir/{network}/client.pem`.
    #[arg(long)]
    pub cln_grpc_client_cert: FileOrCert,

    /// Client key for grpc access. Can either be a file path or the key
    /// contents. Typically stored in `lightningd-dir/{network}/client-key.pem`.
    #[arg(long)]
    pub cln_grpc_client_key: FileOrCert,

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
    #[arg(long, default_value = "60")]
    pub chain_poll_interval_seconds: u64,

    /// Polling interval between redeem runs.
    #[arg(long, default_value = "60")]
    pub redeem_poll_interval_seconds: u64,

    /// Polling interval between checking for uncaught preimages.
    #[arg(long, default_value = "60")]
    pub preimage_poll_interval_seconds: u64,

    /// Automatically apply migrations to the database.
    #[arg(long)]
    pub auto_migrate: bool,

    /// Url to whatthefee.io.
    #[arg(long, default_value = "https://whatthefee.io/data.json")]
    pub whatthefee_url: Url,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    tracing_subscriber::registry()
        .with(EnvFilter::new(&args.log_level))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
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
    let cln_ca_cert = Certificate::from_pem(args.cln_grpc_ca_cert.resolve().await);
    let cln_client_cert = args.cln_grpc_client_cert.resolve().await;
    let cln_client_key = args.cln_grpc_client_key.resolve().await;
    let cln_identity = Identity::from_pem(cln_client_cert, cln_client_key);
    let cln_conn = cln::ClientConnection {
        address: args.cln_grpc_address,
        ca_cert: cln_ca_cert,
        identity: cln_identity,
    };
    let cln_client = Arc::new(cln::Client::new(cln_conn, args.network));
    let pgpool = Arc::new(
        PgPool::connect(&args.db_url)
            .await
            .map_err(|e| format!("failed to connect to postgres: {:?}", e))?,
    );
    if args.auto_migrate {
        postgresql::migrate(&pgpool).await?;
    }
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
    let redeem_repository = Arc::new(postgresql::RedeemRepository::new(
        Arc::clone(&pgpool),
        args.network,
    ));
    let chain_filter = Arc::new(ChainFilterImpl::new(
        Arc::clone(&chain_client),
        Arc::clone(&chain_filter_repository),
    ));
    let fee_estimator_1 = WhatTheFeeEstimator::new(args.whatthefee_url, args.lock_time);
    fee_estimator_1.start().await?;
    let fee_estimator_2 = bitcoind::FeeEstimator::new(Arc::clone(&chain_client));
    let fee_estimator = Arc::new(FallbackFeeEstimator::new(fee_estimator_1, fee_estimator_2));
    let swapper_server = SwapperServer::new(SwapServer::new(SwapServerParams {
        network: args.network,
        max_swap_amount_sat: args.max_swap_amount_sat,
        min_confirmations: args.min_confirmations,
        min_redeem_blocks: args.min_redeem_blocks,
        chain_service: Arc::clone(&chain_client),
        chain_filter_service: Arc::clone(&chain_filter),
        chain_repository: Arc::clone(&chain_repository),
        lightning_client: Arc::clone(&cln_client),
        swap_service: Arc::clone(&swap_service),
        swap_repository: Arc::clone(&swap_repository),
        fee_estimator: Arc::clone(&fee_estimator),
    }));

    let redeem_service = Arc::new(RedeemService::new(
        Arc::clone(&chain_repository),
        Arc::clone(&swap_repository),
    ));
    let token = CancellationToken::new();
    let internal_server = SwapManagerServer::new(internal_server::Server::new(
        args.network,
        Arc::clone(&chain_client),
        chain_filter_repository,
        Arc::clone(&chain_repository),
        Arc::clone(&redeem_service),
        Arc::clone(&swap_repository),
        token.clone(),
    ));
    let chain_monitor = ChainMonitor::new(
        args.network,
        Arc::clone(&chain_client),
        Arc::clone(&chain_repository),
        Duration::from_secs(args.chain_poll_interval_seconds),
    );
    let redeem_monitor = RedeemMonitor::new(RedeemMonitorParams {
        chain_client: Arc::clone(&chain_client),
        fee_estimator: Arc::clone(&fee_estimator),
        poll_interval: Duration::from_secs(args.redeem_poll_interval_seconds),
        swap_repository: Arc::clone(&swap_repository),
        swap_service: Arc::clone(&swap_service),
        redeem_repository: Arc::clone(&redeem_repository),
        redeem_service: Arc::clone(&redeem_service),
        wallet: Arc::clone(&cln_client),
    });
    let preimage_monitor = PreimageMonitor::new(
        Arc::clone(&chain_repository),
        Arc::clone(&cln_client),
        Duration::from_secs(args.preimage_poll_interval_seconds),
        Arc::clone(&swap_repository),
    );

    let server_token = token.clone();
    let internal_server_token = token.clone();
    let chain_monitor_token = token.clone();
    let redeem_monitor_token = token.clone();
    let preimage_monitor_token = token.clone();
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
        info!("Starting redeem monitor");
        let res = redeem_monitor
            .start(async {
                redeem_monitor_token.cancelled().await;
                info!("redeem monitor shutting down");
            })
            .await;
        match res {
            Ok(_) => info!("redeem monitor exited"),
            Err(e) => info!("redeem monitor exited with {:?}", e),
        };
        redeem_monitor_token.cancel();
    });
    tracker.spawn(async move {
        info!("Starting preimage monitor");
        let res = preimage_monitor
            .start(async {
                preimage_monitor_token.cancelled().await;
                info!("preimage monitor shutting down");
            })
            .await;
        match res {
            Ok(_) => info!("preimage monitor exited"),
            Err(e) => info!("preimage monitor exited with {:?}", e),
        };
        preimage_monitor_token.cancel();
    });
    tracker.spawn(async move {
        info!("Starting chain monitor");
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

    info!("swapd started");
    tracker.wait().await;
    info!("shutdown complete");
    Ok(())
}
