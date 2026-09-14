use std::{fs::File, os::fd::AsFd, path::Path, process::Command};

use nix::{
    sched::{CloneFlags, setns},
    unistd::Pid,
};

use crate::error::{KuroError, Result};

pub struct NetMgr;

impl NetMgr {
    /// Configure network interface inside child PID's network namespace
    pub fn setup_network() -> Result<()> {
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
