use std::{collections::HashMap, path::PathBuf};

use crate::{
    cli::commands::{CliArgs, Commands},
    config::spec::load_spec,
    container::{
        builder::CBuilder,
        cleanup::ContainerCleanup,
        state::{ContainerState, ContainerStatus},
    },
    error::{KuroError, Result, validate_id},
    sync::{fifo::ExecFifo, pipe::SyncPipe},
};

pub fn handle_commands(args: &CliArgs) -> Result<()> {
    let mut spipe: Option<SyncPipe> = None;

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

            // Run container builder
            let mut builder = CBuilder::new(container_id.to_owned(), bundle_path, &spec);
            match builder.create() {
                Ok((pid, start_pipe)) => {
                    println!(
                        "[kuro] Container {} created successfully with PID {}",
                        container_id,
                        pid.to_string()
                    );
                    spipe = Some(start_pipe);
                }
                Err(e) => {
                    eprintln!("[kuro] {}", e);
                    ContainerCleanup::new(container_id.as_str(), builder.pid).cleanup()?;

                    return Err(e);
                }
            }
        }

        // // Start
        Commands::Start { container_id } => {
            validate_id(&container_id)?;

            let mut state = ContainerState::load(&container_id)?;
            if state.status != ContainerStatus::Created {
                return Err(KuroError::Start(format!(
                    "Container '{}' is in {:?} state, expected Created",
                    &container_id, state.status
                )));
            }
            if spipe.is_none() {
                return Err(KuroError::Start(format!(
                    "Container '{}' not created properly",
                    &container_id
                )));
            }

            let spec = load_spec(&PathBuf::from(&state.bundle))?;
            // TODO: Execute startContainer hooks (container)
            // CBuilder::run_hook(&spec, &container_id, "startContainer");

            // Signal PID 1 to start container
            // ExecFifo::signal_start(&state_dir)?;
            spipe.unwrap().send_signal()?;

            // Update status -> Running
            state.status = ContainerStatus::Running;
            state.save()?;

            // TODO: Execute poststart hooks (runtime)
            CBuilder::run_hook(&spec, &container_id, "poststart")?;

            println!("Started container {}...", container_id);
        }

        // // State
        Commands::State { container_id } => {
            validate_id(&container_id)?;

            let state = ContainerState::load(container_id)?;
            println!("{}", serde_json::to_string_pretty(&state).unwrap());
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

            println!("[kuro] Deleting container {}...", container_id);

            let state = ContainerState::load(&container_id).ok();
            let pid = state.as_ref().map(|s| s.pid);

            ContainerCleanup::new(container_id, pid).cleanup()?;

            // Poststop hooks
            if let Some(s) = &state {
                let bundle_path = PathBuf::from(&s.bundle);
                if let Ok(spec) = load_spec(&bundle_path) {
                    // let builder = CBuilder::new(container_id.to_owned(), bundle_path, &spec);
                    // builder.run_hook("poststop")?;
                    CBuilder::run_hook(&spec, &container_id, "poststop")?;
                }
            }

            return Ok(());
        }
    }

    Ok(())
}
