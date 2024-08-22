use std::{fs::File, io::{BufRead, BufReader}, path::PathBuf};

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    /// Connectionstring to the postgres database.
    #[arg(long)]
    pub db_url: String,

    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand)]
enum Command {
    /// Commands to add addressfilters
    AddressFilters {
        #[command(subcommand)]
        command: AddressFiltersCommand
    }
}

#[derive(Subcommand)]
enum AddressFiltersCommand {
    /// Adds address filters from a file. 
    Add {
        /// File containing addresses. The file should contain a list of 
        /// addresses to filter, separated by newlines.
        file: PathBuf
    }
}

fn main() {
    let args = Args::parse();
    match args.command {
        Command::AddressFilters { command } => AddressFilterHandler::new(args.db_url).execute(command),
    }
}

struct AddressFilterHandler {
    db_url: String
}

impl AddressFilterHandler {
    fn new(db_url: String) -> Self {
        Self { db_url }
    }

    fn execute(&self, command: AddressFiltersCommand) {
        match command {
            AddressFiltersCommand::Add { file } => self.add_address_filters(file),
        }
    }

    fn add_address_filters(&self, file: PathBuf) {
        let file = File::open(file).expect("no such file");
        let reader = BufReader::new(file);
        let lines = reader.lines();
        
    }
}