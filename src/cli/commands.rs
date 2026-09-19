use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kuro", about = "")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    // --- OCI-COMPLIANT MANDATORY COMMANDS ---

    // Create container
    Create {
        #[arg(long)]
        bundle: PathBuf,

        #[arg(long = "pid-file")]
        pid_file: Option<PathBuf>,

        #[arg(long = "console-socket")]
        console_socket: Option<PathBuf>,

        container_id: String,
    },

    // Start container
    Start {
        container_id: String,
    },

    // Return container state
    State {
        container_id: String,
    },

    // Kill container
    Kill {
        container_id: String,
        signal: String,
    },

    // Delete container
    Delete {
        #[arg(short, long)]
        force: bool,

        container_id: String,
    },
}
