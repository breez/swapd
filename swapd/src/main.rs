use std::{fmt::Debug, path::PathBuf, sync::Arc, time::Duration};

use bitcoin::Network;
use bitcoind::BitcoindClient;
use chain::{ChainMonitor, FallbackFeeEstimator};
use chain_filter::ChainFilterImpl;
use claim::{ClaimMonitor, ClaimMonitorParams, ClaimService, PreimageMonitor};
use clap::Parser;
use figment::{
    providers::{Env, Format, Serialized, Yaml},
    Figment,
};
use internal_server::internal_swap_api::swap_manager_server::SwapManagerServer;
use lightning::LightningClient;
use postgresql::LndRepository;
use public_server::{swap_api::swapper_server::SwapperServer, SwapServer, SwapServerParams};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::{PgPool, Pool, Postgres};
use swap::{HistoricalPaymentMonitor, RandomPrivateKeyProvider, RingRandomProvider, SwapService};
use tokio::signal;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tonic::transport::{Certificate, Identity, Server, Uri};
use tracing::{field, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use wallet::Wallet;
use whatthefee::WhatTheFeeEstimator;

mod bitcoind;
mod chain;
mod chain_filter;
mod claim;
mod cln;
mod internal_server;
mod lightning;
mod lnd;
mod postgresql;
mod public_server;
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

impl Serialize for FileOrCert {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FileOrCert {
    fn deserialize<D>(deserializer: D) -> Result<FileOrCert, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(FileOrCert(s))
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

#[serde_as]
#[derive(Clone, Parser, Debug, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "swapd.conf")]
    pub config: PathBuf,

    /// Address the grpc server will listen on.
    #[arg(long, default_value = "127.0.0.1:58049")]
    pub address: core::net::SocketAddr,

    /// Address the internal grpc server will listen on.
    #[arg(long, default_value = "127.0.0.1:58050")]
    pub internal_address: core::net::SocketAddr,

    /// Maximum amount allowed for swaps.
    #[arg(long, default_value = "4000000")]
    pub max_swap_amount_sat: u64,

    /// Locktime for swaps. This is the time between creation of the swap
    /// address until the client can get a refund. The swap address will contain
    /// a script with an absolute locktime which is the current height + lock
    /// time.
    #[arg(long, default_value = "1008")]
    pub lock_time: u16,

    /// Minimum number of confirmations required before a swap is eligible for
    /// payout.
    #[arg(long, default_value = "1")]
    pub min_confirmations: u64,

    /// Minimum number of blocks needed to claim a utxo onchain. A utxo is no
    /// longer eligible for payout when the lock time left is less than this
    /// number.
    #[arg(long, default_value = "72")]
    pub min_claim_blocks: u32,

    /// Bitcoin network. Valid values are bitcoin, testnet, signet, regtest.
    #[arg(long, default_value = "bitcoin")]
    #[serde_as(as = "DisplayFromStr")]
    pub network: Network,

    /// Amount of satoshis below which an output is considered dust.
    #[arg(long, default_value = "546")]
    pub dust_limit_sat: u64,

    /// cln only: Address to the cln grpc api.
    #[arg(long)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub cln_grpc_address: Option<Uri>,

    /// cln only: Client key for grpc access. Can either be a file path or the
    /// key contents. Typically stored in
    /// `lightningd-dir/{network}/client-key.pem`.
    #[arg(long)]
    pub cln_grpc_ca_cert: Option<FileOrCert>,

    /// cln only: Client cert for grpc access. Can either be a file path or the
    /// cert contents. Typically stored in
    /// `lightningd-dir/{network}/client.pem`.
    #[arg(long)]
    pub cln_grpc_client_cert: Option<FileOrCert>,

    /// cln only: Client key for grpc access. Can either be a file path or the
    /// key contents. Typically stored in
    /// `lightningd-dir/{network}/client-key.pem`.
    #[arg(long)]
    pub cln_grpc_client_key: Option<FileOrCert>,

    /// lnd only: Address to the lnd grpc api.
    #[arg(long)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub lnd_grpc_address: Option<Uri>,

    /// lnd only: CA cert for grpc access. Can either be a file path or the
    /// cert contents. Typically stored in `lnd-dir/ca.cert`.
    #[arg(long)]
    pub lnd_grpc_ca_cert: Option<FileOrCert>,

    /// lnd only: Macaroon for grpc access. Can either be a file path or the
    /// macaroon contents. The macaroon needs offchain:read, offchain:write and
    /// address:write permissions.
    #[arg(long)]
    pub lnd_grpc_macaroon: Option<FileOrCert>,

    /// Loglevel to use. Can be used to filter loges through the env filter
    /// format.
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Connectionstring to the postgres database.
    #[arg(long, default_value = "")]
    pub db_url: String,

    /// Address to the bitcoind rpc.
    #[arg(long, default_value = "http://localhost:8332")]
    pub bitcoind_rpc_address: String,

    /// Bitcoind rpc username.
    #[arg(long, default_value = "")]
    pub bitcoind_rpc_user: String,

    /// Bitcoind rpc password.
    #[arg(long, default_value = "")]
    pub bitcoind_rpc_password: String,

    /// Polling interval between chain syncs.
    #[arg(long, default_value = "60")]
    pub chain_poll_interval_seconds: u64,

    /// Polling interval between claim runs.
    #[arg(long, default_value = "60")]
    pub claim_poll_interval_seconds: u64,

    /// Polling interval between checking historical payment states.
    #[arg(long, default_value = "120")]
    pub payment_poll_interval_seconds: u64,

    /// Polling interval between checking for uncaught preimages.
    #[arg(long, default_value = "60")]
    pub preimage_poll_interval_seconds: u64,

    /// Polling interval between checking whatthefee.io fees.
    #[arg(long, default_value = "60")]
    pub whatthefee_poll_interval_seconds: u64,

    /// Automatically apply migrations to the database.
    #[arg(long)]
    pub auto_migrate: bool,

    /// Url to whatthefee.io.
    #[arg(long, default_value = "https://whatthefee.io/data.json")]
    #[serde_as(as = "DisplayFromStr")]
    pub whatthefee_url: Url,

    /// If this flag is set, the claim logic will not run in this process. It
    /// should then be run separately.
    #[arg(long)]
    pub no_claim: bool,

    /// If this flag is set, the chain sync will not run in this process. It
    /// should then be run separately.
    #[arg(long)]
    pub no_chain: bool,

    /// If this flag is set, the preimage monitor will not run in this process.
    /// It should then be run separately.
    #[arg(long)]
    pub no_preimage: bool,

    /// If this flag is set, the servers will not run in this process. They
    /// should then be run separately.
    #[arg(long)]
    pub no_servers: bool,

    /// Base fee component of the maximum payment fee. The max fee is calculated
    /// as `pay_fee_limit_base + (amount_msat * pay_fee_limit_ppm / 1_000_000)`.
    #[arg(long, default_value = "5000")]
    pub pay_fee_limit_base_msat: u64,

    /// Fee rate component of the maximum payment fee. The max fee is calculated
    /// as `pay_fee_limit_base + (amount_msat * pay_fee_limit_ppm / 1_000_000)`.
    #[arg(long, default_value = "4000")]
    pub pay_fee_limit_ppm: u64,

    /// Payment timeout in seconds.
    #[arg(long, default_value = "120")]
    pub pay_timeout_seconds: u16,

    /// Minimum viable cltv for payout.
    #[arg(long, default_value = "40")]
    pub min_viable_cltv: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config_file = std::fs::canonicalize(&args.config).ok();
    let mut figment = Figment::new().merge(Serialized::defaults(args));
    if let Some(config_file) = &config_file {
        figment = figment.merge(Yaml::file(config_file));
    }

    let args: Args = figment.merge(Env::prefixed("SWAPD_")).extract()?;

    tracing_subscriber::registry()
        .with(EnvFilter::new(&args.log_level))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .init();

    match &config_file {
        Some(config_file) => info!("starting swapd with config file: {}", config_file.display()),
        None => info!("starting swapd without config file"),
    }

    let pgpool = Arc::new(
        PgPool::connect(&args.db_url)
            .await
            .map_err(|e| format!("failed to connect to postgres: {:?}", e))?,
    );
    if args.auto_migrate {
        postgresql::migrate(&pgpool).await?;
    }

    match (&args.cln_grpc_address, &args.lnd_grpc_address) {
        (Some(_), Some(_)) => Err("cannot have both cln and lnd nodes")?,
        (None, None) => Err("a cln or lnd connection needs to be configured")?,
        (Some(cln_grpc_address), None) => {
            let cln_grpc_ca_cert = match &args.cln_grpc_ca_cert {
                Some(c) => c,
                None => Err("missing required arg cln_grpc_ca_cert")?,
            };
            let cln_grpc_client_cert = match &args.cln_grpc_client_cert {
                Some(c) => c,
                None => Err("missing required arg cln_grpc_client_cert")?,
            };
            let cln_grpc_client_key = match &args.cln_grpc_client_key {
                Some(c) => c,
                None => Err("missing required arg cln_grpc_client_key")?,
            };
            let cln_ca_cert = Certificate::from_pem(cln_grpc_ca_cert.resolve().await);
            let cln_client_cert = cln_grpc_client_cert.resolve().await;
            let cln_client_key = cln_grpc_client_key.resolve().await;
            let cln_identity = Identity::from_pem(cln_client_cert, cln_client_key);
            let cln_conn = cln::ClientConnection {
                address: cln_grpc_address.clone(),
                ca_cert: cln_ca_cert,
                identity: cln_identity,
            };
            let cln_client = Arc::new(cln::Client::new(cln_conn, args.network));
            run_with_client(cln_client, pgpool, args).await?;
        }
        (None, Some(lnd_grpc_address)) => {
            let lnd_grpc_ca_cert = match &args.lnd_grpc_ca_cert {
                Some(c) => c,
                None => Err("missing required arg lnd_grpc_tls_cert")?,
            };
            let lnd_grpc_macaroon = match &args.lnd_grpc_macaroon {
                Some(c) => c,
                None => Err("missing required arg lnd_grpc_macaroon")?,
            };
            let lnd_ca_cert = lnd_grpc_ca_cert.resolve().await;
            let lnd_macaroon = lnd_grpc_macaroon.resolve().await;
            let lnd_conn = lnd::ClientConnection {
                address: lnd_grpc_address.clone(),
                macaroon: lnd_macaroon,
                ca_cert: Certificate::from_pem(lnd_ca_cert),
            };
            let lnd_repository = Arc::new(LndRepository::new(Arc::clone(&pgpool)));
            let lnd_client = Arc::new(lnd::Client::new(lnd_conn, args.network, lnd_repository)?);
            run_with_client(lnd_client, pgpool, args).await?;
        }
    };

    Ok(())
}

async fn run_with_client<T>(
    lightning_client: Arc<T>,
    pgpool: Arc<Pool<Postgres>>,
    args: Args,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: LightningClient + Wallet + Send + Sync + Debug + 'static,
{
    let random_provider = Arc::new(RingRandomProvider::new());
    let privkey_provider = RandomPrivateKeyProvider::new(Arc::clone(&random_provider));
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
    let claim_repository = Arc::new(postgresql::ClaimRepository::new(
        Arc::clone(&pgpool),
        args.network,
    ));
    let chain_filter = Arc::new(ChainFilterImpl::new(
        Arc::clone(&chain_client),
        Arc::clone(&chain_filter_repository),
    ));
    let fee_estimator_1 = WhatTheFeeEstimator::new(
        args.whatthefee_url,
        args.lock_time,
        Duration::from_secs(args.whatthefee_poll_interval_seconds),
    );
    fee_estimator_1.start().await?;
    let fee_estimator_2 = bitcoind::FeeEstimator::new(Arc::clone(&chain_client));
    let fee_estimator = Arc::new(FallbackFeeEstimator::new(fee_estimator_1, fee_estimator_2));

    let claim_service = Arc::new(ClaimService::new(
        Arc::clone(&chain_client),
        Arc::clone(&chain_repository),
        Arc::clone(&claim_repository),
        Arc::clone(&swap_repository),
        Arc::clone(&swap_service),
    ));

    let token = CancellationToken::new();
    let signal_token = token.clone();
    let tracker = TaskTracker::new();
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {}
            Err(err) => {
                warn!("Unable to listen for shutdown signal: {}", err);
            }
        }
        signal_token.cancel();
    });
    let mut payment_monitor = HistoricalPaymentMonitor::new(
        Arc::clone(&lightning_client),
        Duration::from_secs(args.payment_poll_interval_seconds),
        Arc::clone(&swap_repository),
    );
    match payment_monitor.initialize().await {
        Ok(_) => {
            let payment_monitor_token = token.clone();
            tracker.spawn(async move {
                info!("Starting historical payment monitor");
                let res = payment_monitor
                    .start(payment_monitor_token.child_token())
                    .await;
                match res {
                    Ok(_) => info!("historical payment monitor exited"),
                    Err(e) => info!("historical payment monitor exited with {:?}", e),
                };
            });
        }
        Err(e) => warn!("failed to initialize historical payment monitor: {:?}, continuing without processing historical payments.", e),
    }

    if !args.no_claim {
        let claim_monitor_token = token.clone();
        let claim_monitor = ClaimMonitor::new(ClaimMonitorParams {
            chain_client: Arc::clone(&chain_client),
            fee_estimator: Arc::clone(&fee_estimator),
            poll_interval: Duration::from_secs(args.claim_poll_interval_seconds),
            claim_repository: Arc::clone(&claim_repository),
            claim_service: Arc::clone(&claim_service),
            wallet: Arc::clone(&lightning_client),
        });
        tracker.spawn(async move {
            info!("Starting claim monitor");
            let res = claim_monitor.start(claim_monitor_token.child_token()).await;
            match res {
                Ok(_) => info!("claim monitor exited"),
                Err(e) => info!("claim monitor exited with {:?}", e),
            };
            claim_monitor_token.cancel();
        });
    }
    if !args.no_preimage {
        let preimage_monitor_token = token.clone();
        let preimage_monitor = PreimageMonitor::new(
            Arc::clone(&chain_repository),
            Arc::clone(&lightning_client),
            Duration::from_secs(args.preimage_poll_interval_seconds),
            Arc::clone(&swap_repository),
        );
        tracker.spawn(async move {
            info!("Starting preimage monitor");
            let res = preimage_monitor
                .start(preimage_monitor_token.child_token())
                .await;
            match res {
                Ok(_) => info!("preimage monitor exited"),
                Err(e) => info!("preimage monitor exited with {:?}", e),
            };
            preimage_monitor_token.cancel();
        });
    }
    if !args.no_chain {
        let chain_monitor_token = token.clone();
        let chain_monitor = Arc::new(ChainMonitor::new(
            args.network,
            Arc::clone(&chain_client),
            Arc::clone(&chain_repository),
            Duration::from_secs(args.chain_poll_interval_seconds),
        ));
        tracker.spawn(async move {
            info!("Starting chain monitor");
            let res = chain_monitor.start(chain_monitor_token.child_token()).await;
            match res {
                Ok(_) => info!("chain monitor exited"),
                Err(e) => info!("chain monitor exited with {:?}", e),
            };
            chain_monitor_token.cancel();
        });
    }
    if !args.no_servers {
        let server_token = token.clone();
        let swapper_server = SwapperServer::new(SwapServer::new(SwapServerParams {
            network: args.network,
            max_swap_amount_sat: args.max_swap_amount_sat,
            min_confirmations: args.min_confirmations,
            min_claim_blocks: args.min_claim_blocks,
            min_viable_cltv: args.min_viable_cltv,
            pay_fee_limit_base_msat: args.pay_fee_limit_base_msat,
            pay_fee_limit_ppm: args.pay_fee_limit_ppm,
            pay_timeout_seconds: args.pay_timeout_seconds,
            chain_service: Arc::clone(&chain_client),
            chain_filter_service: Arc::clone(&chain_filter),
            chain_repository: Arc::clone(&chain_repository),
            lightning_client: Arc::clone(&lightning_client),
            random_provider: Arc::clone(&random_provider),
            swap_service: Arc::clone(&swap_service),
            swap_repository: Arc::clone(&swap_repository),
            fee_estimator: Arc::clone(&fee_estimator),
        }));
        tracker.spawn(async move {
            info!(
                address = field::display(&args.address),
                "Starting swapper server"
            );
            let res = Server::builder()
                .add_service(swapper_server)
                .serve_with_shutdown(args.address, server_token.cancelled())
                .await;
            match res {
                Ok(_) => info!("swapper server exited"),
                Err(e) => info!("swapper server exited with {:?}", e),
            }

            server_token.cancel();
        });

        let internal_server_token = token.clone();
        let internal_server = SwapManagerServer::new(internal_server::Server::new(
            internal_server::ServerParams {
                chain_client: Arc::clone(&chain_client),
                chain_filter_repository: Arc::clone(&chain_filter_repository),
                chain_repository: Arc::clone(&chain_repository),
                fee_estimator: Arc::clone(&fee_estimator),
                swap_repository: Arc::clone(&swap_repository),
                wallet: Arc::clone(&lightning_client),
                network: args.network,
                claim_service: Arc::clone(&claim_service),
                token: token.clone(),
            },
        ));
        tracker.spawn(async move {
            info!(
                address = field::display(&args.internal_address),
                "Starting internal server"
            );
            let res = Server::builder()
                .add_service(internal_server)
                .serve_with_shutdown(args.internal_address, internal_server_token.cancelled())
                .await;
            match res {
                Ok(_) => info!("internal server exited"),
                Err(e) => info!("internal server exited with {:?}", e),
            }
            internal_server_token.cancel();
        });
    }

    info!("swapd started");

    // Ensure the tracker completes when all tasks have completed.
    tracker.close();

    // Wait for all background tasks to complete.
    tracker.wait().await;
    info!("shutdown complete");
    Ok(())
}
