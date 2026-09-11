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
        container_id: String,
        bundle_path: String,
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
        container_id: String,
    },
}
