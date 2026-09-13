use std::{fs::File, os::fd::AsFd, path::Path, process::Command};

use nix::{
    sched::{CloneFlags, setns},
    unistd::Pid,
};

use crate::error::{KuroError, Result};

pub struct NetMgr;

impl NetMgr {
    /// Configure network interface inside child PID's network namespace
    pub fn setup_network(child_pid: Pid, netns_path: Option<&Path>) -> Result<()> {
        if let Some(netns) = netns_path {
            // TODO: Attach provided path as network namespace
            let nspath = File::open(netns)
                .map_err(|e| KuroError::Netns(format!("Failed to open netns path: {}", e)))?;
            setns(nspath.as_fd(), CloneFlags::CLONE_NEWNET).map_err(|e| {
                KuroError::Netns(format!(
                    "Failed to set namespace to path {:?}: {}",
                    netns, e
                ))
            })?;

            return Ok(());
        }

        // TODO: New ns to be created and setup here (no existing path provided)
        Self::bring_up_loopback()?;

        Ok(())
    }

    /// Bring up loopback device in new namespace
    fn bring_up_loopback() -> Result<()> {
        let status = Command::new("ip")
            .args(&["link", "set", "dev", "lo", "up"])
            .status()?;

        if !status.success() {
            return Err(KuroError::Netns(
                "Failed to bring up loopback device".to_string(),
            ));
        }

        Ok(())
    }
}
