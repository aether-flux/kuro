use std::{collections::HashMap, path::PathBuf};

use crate::{
    cli::commands::{CliArgs, Commands},
    config::spec::load_spec,
    container::state::{ContainerState, ContainerStatus},
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

            // Validate bundle path
            let bundle_path = PathBuf::from(bundle_path);
            if !bundle_path.exists() || !bundle_path.is_dir() {
                return Err(KuroError::InvalidArgs(format!(
                    "bundle_path '{}' does not exist or is not a directory",
                    bundle_path.display()
                )));
            }

            // Convert bundle path to absolute path
            let bundle = bundle_path.canonicalize().map_err(|e| {
                KuroError::InvalidArgs(format!(
                    "Failed to resolve absolute path for bundle '{}': {}",
                    bundle_path.display(),
                    e
                ))
            })?;

            // Load spec config.json
            let spec = load_spec(&bundle)?;

            // Check existence of rootfs directory
            let root = spec
                .root()
                .as_ref()
                .ok_or_else(|| KuroError::InvalidBundle {
                    path: bundle.clone(),
                    reason: "config.json is missing the 'root' section".to_string(),
                })?;
            let rootfs_rel_path = root.path();
            let rootfs_full_path = bundle.join(rootfs_rel_path);
            if !rootfs_full_path.exists() {
                return Err(KuroError::InvalidArgs(format!(
                    "bundle_path does not contain rootfs directory at '{}'",
                    rootfs_rel_path.display()
                )));
            }

            // Get annotations
            let annotations = spec.annotations().as_ref().cloned().unwrap_or_default();

            // Create initial state
            let state = ContainerState {
                oci_version: spec.version().to_owned(),
                id: container_id.to_owned(),
                status: ContainerStatus::Creating,
                pid: -1,
                bundle: bundle.to_string_lossy().into_owned(),
                annotations,
            };

            // Save initial state to disk
            state.save()?;
            println!(
                "Container '{}' state initialized with Creating",
                container_id
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
