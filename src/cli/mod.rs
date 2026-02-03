use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "toy")]
#[command(version, about = "A toy language JIT compiler and runner", long_about = None)]
pub struct Cli {
    /// The .toy script file to run
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Run integration tests
    #[arg(short, long)]
    pub test: bool,

    /// Set verbose level
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
