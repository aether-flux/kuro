use std::{collections::HashMap, fs, path::PathBuf};

use nix::{sys::signal::kill, unistd::Pid};
use serde::{Deserialize, Serialize};

use crate::error::{KuroError, Result};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ContainerStatus {
    Creating,
    Created,
    Running,
    Stopped,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContainerState {
    pub oci_version: String,
    pub id: String,
    pub status: ContainerStatus,
    pub pid: i32,
    pub bundle: String,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

impl ContainerState {
    /// Get root state directory (root vs rootless modes)
    pub fn get_state_dir(container_id: &str) -> PathBuf {
        let uid = unsafe { libc::geteuid() };
        let base_run = if uid == 0 {
            // runtime dir for root user
            PathBuf::from("/run/kuro")
        } else {
            // runtime dir per user
            std::env::var("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(format!("/run/user/{}", uid)))
                .join("kuro")
        };

        base_run.join(container_id)
    }

    /// Get state persistence file path
    pub fn get_state_file(container_id: &str) -> PathBuf {
        Self::get_state_dir(container_id).join("state.json")
    }

    /// Save current contents to file
    pub fn save(&self) -> Result<()> {
        let dir = Self::get_state_dir(&self.id);
        fs::create_dir_all(&dir)?;
        let data = serde_json::to_string_pretty(self)?;
        fs::write(Self::get_state_file(&self.id), data)?;
        Ok(())
    }

    /// Load contents of state file to struct
    pub fn load(container_id: &str) -> Result<Self> {
        // Check if state file exists
        let path = Self::get_state_file(container_id);
        if !path.exists() {
            return Err(KuroError::ContainerStateNotFound {
                id: container_id.to_string(),
            });
        }

        // Convert file contents into ContainerState object
        let data = fs::read_to_string(path)?;
        let mut state: ContainerState = serde_json::from_str(&data)?;

        // Check if host PID is still alive
        if state.status == ContainerStatus::Running || state.status == ContainerStatus::Created {
            if state.pid > 0 {
                // signal None (0) checks if process exists without sending any signal
                if kill(Pid::from_raw(state.pid), None).is_err() {
                    state.status = ContainerStatus::Stopped;
                    let _ = state.save();
                }
            }
        }

        Ok(state)
    }

    /// Cleanup state file
    pub fn destroy(container_id: &str) -> Result<()> {
        let dir = Self::get_state_dir(container_id);
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }

        Ok(())
    }
}
