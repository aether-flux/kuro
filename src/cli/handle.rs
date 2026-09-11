use crate::{
    cli::commands::{CliArgs, Commands},
    error::{KuroError, Result},
};

pub fn handle_commands(args: &CliArgs) -> Result<()> {
    match &args.command {
        // --- OCI-Compliant Mandatory Commands ---
        // // Create
        Commands::Create {
            container_id,
            bundle_path,
        } => {
            if container_id.trim().is_empty() {
                return Err(KuroError::InvalidArgs(
                    "container_id cannot be empty".to_string(),
                ));
            }
            if bundle_path.trim().is_empty() {
                return Err(KuroError::InvalidArgs(
                    "bundle_path cannot be empty".to_string(),
                ));
            }

            println!(
                "Creating container {} with bundle {}",
                container_id, bundle_path
            );
        }

        // // Start
        Commands::Start { container_id } => {
            if container_id.trim().is_empty() {
                return Err(KuroError::InvalidArgs(
                    "container_id cannot be empty".to_string(),
                ));
            }

            println!("Starting container {}...", container_id);
        }

        // // State
        Commands::State { container_id } => {
            if container_id.trim().is_empty() {
                return Err(KuroError::InvalidArgs(
                    "container_id cannot be empty".to_string(),
                ));
            }

            println!("State of container {} is 'ded'", container_id);
        }

        // // Kill
        Commands::Kill {
            container_id,
            signal,
        } => {
            if container_id.trim().is_empty() {
                return Err(KuroError::InvalidArgs(
                    "container_id cannot be empty".to_string(),
                ));
            }
            if signal.trim().is_empty() {
                return Err(KuroError::InvalidArgs("signal cannot be empty".to_string()));
            }

            println!("Sending signal {} to container {}", signal, container_id);
        }

        // // Delete
        Commands::Delete { container_id } => {
            if container_id.trim().is_empty() {
                return Err(KuroError::InvalidArgs(
                    "container_id cannot be empty".to_string(),
                ));
            }

            println!("Deleting container {}...", container_id);
        }
    }

    Ok(())
}
