use std::{fs, process::Command};

use nix::unistd::{Pid, geteuid};
use oci_spec::runtime::LinuxIdMapping;

use crate::error::{KuroError, Result};

pub struct UserMgr;

impl UserMgr {
    /// Writes UID and GID mappings
    /// Detects root/rootless modes and works accordingly
    pub fn set_mappings(
        child_pid: Pid,
        uid_mappings: Option<&[LinuxIdMapping]>,
        gid_mappings: Option<&[LinuxIdMapping]>,
    ) -> Result<()> {
        let is_root = geteuid().is_root();

        if let Some(uids) = uid_mappings {
            if !uids.is_empty() {
                Self::write_id_map(child_pid, "uid", uids, is_root)?;
            }
        }

        if let Some(gids) = gid_mappings {
            if !gids.is_empty() {
                // Set setgroups to deny to write directly to /proc/<pid>/gid_map
                if is_root {
                    let setgroups_path = format!("/proc/{}/setgroups", child_pid);
                    let _ = fs::write(setgroups_path, "deny");
                }
                Self::write_id_map(child_pid, "gid", gids, is_root)?;
            }
        }

        Ok(())
    }

    /// Write ID mappings
    pub fn write_id_map(
        child_pid: Pid,
        map_type: &str,
        mappings: &[LinuxIdMapping],
        is_root: bool,
    ) -> Result<()> {
        if is_root {
            // Root mode: Directly write to /proc
            let map_file = format!("/proc/{}/{}_map", child_pid, map_type);
            let mut content = String::new();
            for m in mappings {
                content.push_str(&format!(
                    "{} {} {}\n",
                    m.container_id(),
                    m.host_id(),
                    m.size()
                ));
            }

            fs::write(&map_file, content)
                .map_err(|e| KuroError::Userns(format!("Failed to write {}: {}", map_file, e)))?;
        } else {
            // Rootless mode: Use helpers newuidmap/newgidmap
            let helper = format!("new{}map", map_type);
            let mut cmd = Command::new(&helper);
            cmd.arg(child_pid.to_string());

            for m in mappings {
                cmd.arg(m.container_id().to_string())
                    .arg(m.host_id().to_string())
                    .arg(m.size().to_string());
            }

            let status = cmd.status().map_err(|e| {
                KuroError::Userns(format!("Failed to execute helper {}: {}", helper, e))
            })?;

            if !status.success() {
                return Err(KuroError::Userns(format!(
                    "{} helper failed with status {}",
                    helper, status
                )));
            }
        }

        Ok(())
    }
}
