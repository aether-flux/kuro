use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    os::fd::FromRawFd,
    path::PathBuf,
    str::FromStr,
};

use nix::{
    sys::{
        signal::{Signal, kill},
        termios::{SetArg, cfmakeraw, tcgetattr, tcsetattr},
    },
    unistd::Pid,
};

use crate::{
    cli::commands::{CliArgs, Commands},
    config::spec::load_spec,
    container::{
        builder::CBuilder,
        cleanup::ContainerCleanup,
        state::{ContainerState, ContainerStatus},
    },
    error::{KuroError, Result, validate_id},
    sync::{console::ConsoleSocket, fifo::ExecFifo, pipe::SyncPipe},
};

pub fn handle_commands(args: &CliArgs) -> Result<()> {
    match &args.command {
        // --- OCI-Compliant Mandatory Commands ---
        // // Create
        Commands::Create {
            container_id,
            bundle,
            pid_file,
            console_socket,
        } => {
            validate_id(container_id)?;

            if let Ok(_) = ContainerState::load(container_id) {
                return Err(KuroError::ContainerAlreadyExists {
                    id: container_id.to_string(),
                });
            }

            // Validate bundle path
            let bundle_path = PathBuf::from(bundle);
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
                Ok(pid) => println!(
                    "[kuro] Container {} created successfully with PID {}",
                    container_id, pid
                ),
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
                    container_id, state.status
                )));
            }
            if state.pid < 2 {
                return Err(KuroError::Start(format!(
                    "Container '{}' has not been created properly, and has no PID (check by running 'kuro state')",
                    container_id
                )));
            }
            println!("state loaded");

            let spec = load_spec(&PathBuf::from(&state.bundle))?;
            let interactive = spec
                .process()
                .as_ref()
                .and_then(|p| p.terminal())
                .unwrap_or(false);
            println!("spec loaded, interactive = {}", interactive);

            let state_dir = ContainerState::get_state_dir(&container_id);
            let socket_path = state_dir.join("console.sock");

            let console_listener = if interactive {
                Some(ConsoleSocket::listen(&socket_path)?)
            } else {
                None
            };
            println!("console listener: {:?}", console_listener);

            // Signal PID 1 to start container
            ExecFifo::signal_start(&state_dir)?;
            println!("signal sent");

            // Update status -> Running
            state.status = ContainerStatus::Running;
            state.save()?;
            println!("status saved");

            // [x]   Execute poststart hooks (runtime)
            CBuilder::run_hook(&spec, container_id, "poststart")?;

            println!("Started container {}...", container_id);

            if let Some(listener) = console_listener {
                let master_fd = ConsoleSocket::recv_fd(&listener)?;
                println!("received fd");
                let _ = fs::remove_file(&socket_path);

                let master_file = unsafe { fs::File::from_raw_fd(master_fd) };
                let mut master_writer = master_file.try_clone().map_err(|e| {
                    KuroError::ExecFailed(format!("Failed to clone console fd: {}", e))
                })?;
                let mut master_reader = master_file;

                // Send keystrokes to container's shell
                let stdin_handle = std::io::stdin();
                let orig_termios = tcgetattr(&stdin_handle)
                    .map_err(|e| KuroError::ExecFailed(format!("tcgetattr failed: {}", e)))?;
                let mut raw = orig_termios.clone();
                cfmakeraw(&mut raw);
                tcsetattr(&stdin_handle, SetArg::TCSANOW, &raw)
                    .map_err(|e| KuroError::ExecFailed(format!("tcsetattr failed: {}", e)))?;

                // stdin -> container
                std::thread::spawn(move || {
                    let mut stdin = std::io::stdin();
                    let mut buf = [0u8; 4096];
                    loop {
                        match stdin.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if master_writer.write_all(&buf[..n]).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });

                // container -> stdout
                let mut stdout = std::io::stdout();
                let mut buf = [0u8; 4096];
                loop {
                    match master_reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let _ = stdout.write_all(&buf[..n]);
                            let _ = stdout.flush();
                        }
                    }
                }

                let _ = tcsetattr(&stdin_handle, SetArg::TCSANOW, &orig_termios);
                println!("\n[kuro] Container {} exited", container_id);

                state.status = ContainerStatus::Stopped;
                let _ = state.save();
            }
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

            let sig_str = if signal.trim().is_empty() {
                "SIGTERM"
            } else {
                signal.as_str()
            };
            let sig = parse_signal(sig_str)?;
            let state = ContainerState::load(&container_id)?;

            if state.status == ContainerStatus::Stopped {
                return Err(KuroError::InvalidArgs(format!(
                    "Container '{}' is already stopped",
                    &container_id
                )));
            }

            let pid = Pid::from_raw(state.pid);

            kill(pid, sig).map_err(|e| {
                KuroError::ExecFailed(format!(
                    "Failed to send signal {:?} to PID {}: {}",
                    sig, pid, e
                ))
            })?;

            println!("[kuro] Signal {:?} sent to container {}", sig, container_id);
        }

        // // Delete
        Commands::Delete {
            container_id,
            force,
        } => {
            validate_id(container_id)?;

            println!("[kuro] Deleting container {}...", container_id);

            let state = ContainerState::load(container_id).ok();

            if state.is_none() {
                return Err(KuroError::ContainerNotFound {
                    id: container_id.to_string(),
                });
            }

            if state.as_ref().unwrap().status == ContainerStatus::Running && !force {
                return Err(KuroError::InvalidArgs(format!(
                    "Container {} is currently running; use '--force' to kill a running container",
                    container_id
                )));
            }

            let pid = state.as_ref().map(|s| s.pid);
            ContainerCleanup::new(container_id, pid).cleanup()?;

            // Poststop hooks
            if let Some(s) = &state {
                let bundle_path = PathBuf::from(&s.bundle);
                if let Ok(spec) = load_spec(&bundle_path) {
                    // let builder = CBuilder::new(container_id.to_owned(), bundle_path, &spec);
                    // builder.run_hook("poststop")?;
                    CBuilder::run_hook(&spec, container_id, "poststop")?;
                }
            }

            return Ok(());
        }
    }

    Ok(())
}

// Parse signal string to Signal variant
fn parse_signal(sig: &str) -> Result<Signal> {
    let sig_upper = sig.trim().to_uppercase();

    let normalized = if sig_upper.parse::<i32>().is_ok() {
        let num: i32 = sig_upper.parse().unwrap();
        Signal::try_from(num)
            .map_err(|_| KuroError::InvalidArgs(format!("Invalid signal number: {}", num)))?
    } else if sig_upper.starts_with("SIG") {
        Signal::from_str(&sig_upper)
            .map_err(|_| KuroError::InvalidArgs(format!("Invalid signal name: {}", sig)))?
    } else {
        Signal::from_str(&format!("SIG{}", sig_upper))
            .map_err(|_| KuroError::InvalidArgs(format!("Invalid signal name: {}", sig)))?
    };

    Ok(normalized)
}
