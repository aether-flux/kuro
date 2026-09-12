use crate::{
    cli::commands::{CliArgs, Commands},
    error::{KuroError, Result, validate_id},
};

pub fn handle_commands(args: &CliArgs) -> Result<()> {
    match &args.command {
        // --- OCI-Compliant Mandatory Commands ---
        // // Create
        Commands::Create {
            container_id,
            bundle_path,
        } => {
            validate_id(&container_id)?;
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
            validate_id(&container_id)?;

            println!("Starting container {}...", container_id);
        }

        // // State
        Commands::State { container_id } => {
            validate_id(&container_id)?;

            println!("State of container {} is 'ded'", container_id);
        }

        // // Kill
        Commands::Kill {
            container_id,
            signal,
        } => {
            validate_id(&container_id)?;
            if signal.trim().is_empty() {
                return Err(KuroError::InvalidArgs("signal cannot be empty".to_string()));
            }

            println!("Sending signal {} to container {}", signal, container_id);
        }

        // // Delete
        Commands::Delete { container_id } => {
            validate_id(&container_id)?;

            println!("Deleting container {}...", container_id);
        }
    }

    Ok(())
}
