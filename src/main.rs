use clap::Parser;

use crate::{
    cli::{commands::CliArgs, handle::handle_commands},
    error::Result,
};

mod cgroups;
mod cli;
mod config;
mod container;
mod error;
mod namespaces;
mod sync;

fn main() -> Result<()> {
    let args = CliArgs::try_parse().unwrap_or_else(|e| e.exit());
    if let Err(e) = handle_commands(&args) {
        return Err(e);
    }

    Ok(())
}
