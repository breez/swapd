use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use internal_swap_api::{
    swap_manager_client::SwapManagerClient, AddAddressFiltersRequest, GetInfoRequest,
    GetSwapRequest, ListRedeemableRequest,
};
use tonic::{
    transport::{Channel, Uri},
    Request,
};

mod internal_swap_api {
    tonic::include_proto!("swap_internal");
}

fn parse_hex(s: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(s)
}

#[derive(Parser)]
struct Args {
    /// Address to the internal grpc server.
    #[arg(long)]
    pub grpc_uri: Uri,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Commands to add addressfilters
    AddressFilters {
        #[command(subcommand)]
        command: AddressFiltersCommand,
    },
    GetInfo,
    GetSwap {
        #[clap(long)]
        address: Option<String>,
        #[clap(long)]
        payment_request: Option<String>,
        #[clap(long, value_parser(parse_hex))]
        hash: Option<Vec<u8>>,
    },
    ListRedeemable,
}

#[derive(Subcommand)]
enum AddressFiltersCommand {
    /// Adds address filters from a file.
    Add {
        /// File containing addresses. The file should contain a list of
        /// addresses to filter, separated by newlines.
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut client = SwapManagerClient::connect(args.grpc_uri).await?;

    match args.command {
        Command::AddressFilters { command } => {
            AddressFilterHandler::new(client).execute(command).await?
        }
        Command::GetInfo => {
            let resp = client
                .get_info(Request::new(GetInfoRequest::default()))
                .await?
                .into_inner();
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        Command::GetSwap {
            address,
            payment_request,
            hash,
        } => {
            let resp = client
                .get_swap(Request::new(GetSwapRequest {
                    address,
                    payment_request,
                    payment_hash: hash,
                }))
                .await?
                .into_inner();
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        Command::ListRedeemable => {
            let resp = client
                .list_redeemable(Request::new(ListRedeemableRequest::default()))
                .await?
                .into_inner();
            println!("{}", serde_json::to_string_pretty(&resp)?)
        }
    }

    Ok(())
}

struct AddressFilterHandler {
    client: SwapManagerClient<Channel>,
}

impl AddressFilterHandler {
    fn new(client: SwapManagerClient<Channel>) -> Self {
        Self { client }
    }

    async fn execute(
        &mut self,
        command: AddressFiltersCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match command {
            AddressFiltersCommand::Add { file } => self.add_address_filters(file).await?,
        }

        Ok(())
    }

    async fn add_address_filters(
        &mut self,
        file: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(file).expect("no such file");
        let reader = BufReader::new(file);
        let addresses = reader
            .lines()
            .collect::<Result<Vec<String>, std::io::Error>>()?;
        self.client
            .add_address_filters(Request::new(AddAddressFiltersRequest { addresses }))
            .await?;
        Ok(())
    }
}
