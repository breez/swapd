use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use internal_swap_api::{swap_manager_client::SwapManagerClient, AddAddressFiltersRequest, ListRedeemableRequest};
use tonic::{transport::Uri, Request};

mod internal_swap_api {
    tonic::include_proto!("swap_internal");
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
    ListRedeemable
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
    match args.command {
        Command::AddressFilters { command } => {
            AddressFilterHandler::new(args.grpc_uri)
                .execute(command)
                .await?
        },
        Command::ListRedeemable => {
            let mut client = SwapManagerClient::connect(args.grpc_uri).await?;
            let redeemables = client.list_redeemable(Request::new(ListRedeemableRequest::default())).await?;
            println!("{}", serde_json::to_string_pretty(&redeemables.into_inner())?)
        }
    }

    Ok(())
}

struct AddressFilterHandler {
    grpc_uri: Uri,
}

impl AddressFilterHandler {
    fn new(grpc_uri: Uri) -> Self {
        Self { grpc_uri }
    }

    async fn execute(
        &self,
        command: AddressFiltersCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match command {
            AddressFiltersCommand::Add { file } => self.add_address_filters(file).await?,
        }

        Ok(())
    }

    async fn add_address_filters(&self, file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(file).expect("no such file");
        let reader = BufReader::new(file);
        let addresses = reader
            .lines()
            .collect::<Result<Vec<String>, std::io::Error>>()?;
        let mut client = SwapManagerClient::connect(self.grpc_uri.clone()).await?;
        client
            .add_address_filters(Request::new(AddAddressFiltersRequest { addresses }))
            .await?;
        Ok(())
    }
}
