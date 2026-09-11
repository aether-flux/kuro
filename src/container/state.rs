use std::{collections::HashMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

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
    // Get root state directory (root vs rootless modes)
    pub fn get_state_dir(container_id: &str) -> PathBuf {
        let uid = unsafe { libc::geteuid() };
        let base_run = if uid == 0 {
            PathBuf::from("/run/kuro")
        } else {
            std::env::var("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(format!("/run/user/{}", uid)))
                .join("kuro")
        };

        base_run.join(container_id)
    }

    // Get state persistence file path
    pub fn get_state_file(container_id: &str) -> PathBuf {
        Self::get_state_dir(container_id).join("state.json")
    }

    // Save current contents to file
    pub fn save(&self) -> Result<()> {
        let dir = Self::get_state_dir(&self.id);
        fs::create_dir_all(&dir)?;
        let data = serde_json::to_string_pretty(self)?;
        fs::write(Self::get_state_file(&self.id), data)?;
        Ok(())
    }
}
