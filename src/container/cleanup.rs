use std::{fs, path::PathBuf};

use nix::{
    sys::signal::{Signal, kill},
    unistd::{Pid, getpid},
};

use crate::{container::state::ContainerState, error::Result};

pub struct ContainerCleanup {
    pub container_id: String,
    pub pid: Option<i32>,
    pub cgroup_path: PathBuf,
    pub state_dir: PathBuf,
}

impl ContainerCleanup {
    /// Create new cleaner instance
    pub fn new(container_id: &str, pid: Option<i32>) -> Self {
        let cgroup_path = PathBuf::from("/sys/fs/cgroup/kuro").join(container_id);
        let state_dir = ContainerState::get_state_dir(container_id);

        Self {
            container_id: container_id.to_string(),
            pid,
            cgroup_path,
            state_dir,
        }
    }

    /// Cleanup process
    pub fn cleanup(&self) -> Result<()> {
        println!("[kuro] Cleaning up container '{}'...", self.container_id);

        // Force kill child processes if alive
        if let Some(pid_raw) = self.pid {
            let curpid = getpid().as_raw();

            // If pid is 0 or -1, do not kill (container not properly created)
            if pid_raw > 1 && pid_raw != curpid {
                let pid = Pid::from_raw(pid_raw);
                // Send SIGKILL; ignore ESRCH error (process already dead)
                let _ = kill(pid, Signal::SIGKILL);
            } else {
                eprintln!(
                    "[kuro] WARN: Skipping kill for invalid or dangerous PID: {}",
                    pid_raw
                );
            }
        }

        // Remove cgroup directory
        if self.cgroup_path.exists() {
            if let Err(e) = fs::remove_dir(&self.cgroup_path) {
                eprintln!(
                    "[kuro] WARN: Failed to remove cgroup path {:?}: {}",
                    self.cgroup_path, e
                );
            }
        }

        // Remove state directory
        if self.state_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&self.state_dir) {
                eprintln!(
                    "[kuro] WARN: Failed to remove state diirectory {:?}: {}",
                    self.state_dir, e
                );
            }
        }

        Ok(())
    }
}
