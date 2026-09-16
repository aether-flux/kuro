use crate::error::{KuroError, Result};
use oci_spec::runtime::{LinuxDeviceCgroup, LinuxResources};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct CgroupMgr {
    path: PathBuf,
}

impl CgroupMgr {
    /// Setup new container cgroup
    pub fn new(container_id: &str) -> Result<Self> {
        let root_path = PathBuf::from("/sys/fs/cgroup");
        let base_path = root_path.join("kuro");

        if !base_path.exists() {
            std::fs::create_dir_all(&base_path).map_err(|e| {
                KuroError::Cgroup(format!("Failed to create base cgroup dir: {}", e))
            })?;
        }

        // Enable controllers dynamically
        Self::enable_controllers(&root_path, &base_path)?;

        let path = base_path.join(container_id);
        std::fs::create_dir_all(&path).map_err(|e| {
            KuroError::Cgroup(format!("Failed to create container cgroup dir: {}", e))
        })?;

        Ok(Self { path })
    }

    /// Enable controllers
    fn enable_controllers(parent: &Path, target_dir: &Path) -> Result<()> {
        let ctrl_file = parent.join("cgroup.controllers");
        // Fallback if system doesn't support full cgroups v2
        if !ctrl_file.exists() {
            return Ok(());
        }

        let available = std::fs::read_to_string(&ctrl_file).map_err(|e| {
            KuroError::Cgroup(format!("Failed to read available controllers: {}", e))
        })?;

        let subtree_cmd = available
            .split_whitespace()
            .map(|c| format!("+{}", c))
            .collect::<Vec<_>>()
            .join(" ");

        if !subtree_cmd.is_empty() {
            // Ignore errors if enabled/busy
            let _ = std::fs::write(target_dir.join("cgroup.subtree_control"), subtree_cmd);
        }

        Ok(())
    }

    /// Add process to cgroup
    pub fn add_proc(&self, pid: nix::unistd::Pid) -> Result<()> {
        std::fs::write(self.path.join("cgroup.procs"), pid.to_string())
            .map_err(|e| KuroError::Cgroup(format!("Failed to add pid {} to cgroup: {}", pid, e)))
    }

    /// Apply resource limits
    pub fn apply_limits(&self, resources: &LinuxResources) -> Result<()> {
        // Memory limits
        if let Some(mem) = resources.memory() {
            if let Some(limit) = mem.limit() {
                let val = if limit < 0 {
                    "max".to_string()
                } else {
                    limit.to_string()
                };
                std::fs::write(self.path.join("memory.max"), val)
                    .map_err(|e| KuroError::Cgroup(format!("Failed to write memory.max: {}", e)))?;
            }

            if let Some(swap) = mem.swap() {
                let val = if swap < 0 {
                    "max".to_string()
                } else {
                    swap.to_string()
                };
                std::fs::write(self.path.join("memory.swap.max"), val).map_err(|e| {
                    KuroError::Cgroup(format!("Failed to write memory.swap.max: {}", e))
                })?;
            }
        }

        // Pid limits
        if let Some(pids) = resources.pids() {
            let val = if pids.limit() <= 0 {
                "max".to_string()
            } else {
                pids.limit().to_string()
            };
            std::fs::write(self.path.join("pids.max"), val)
                .map_err(|e| KuroError::Cgroup(format!("Failed to write pids.max: {}", e)))?;
        }

        // CPU limits
        if let Some(cpu) = resources.cpu() {
            if let (Some(quota), Some(period)) = (cpu.quota(), cpu.period()) {
                let quota_str = if quota < 0 {
                    "max".to_string()
                } else {
                    quota.to_string()
                };
                let val = format!("{} {}", quota_str, period);
                std::fs::write(self.path.join("cpu.max"), val)
                    .map_err(|e| KuroError::Cgroup(format!("Failed to write cpu.max: {}", e)))?;
            }

            if let Some(shares) = cpu.shares() {
                let weight = 1 + (shares.saturating_sub(2) * 9999) / 262142;
                std::fs::write(self.path.join("cpu.weight"), weight.to_string())
                    .map_err(|e| KuroError::Cgroup(format!("Failed to write cpu.weight: {}", e)))?;
            }
        }

        // CPU Set limits
        if let Some(cpuset) = resources.cpu() {
            if let Some(cpus) = cpuset.cpus() {
                std::fs::write(self.path.join("cpuset.cpus"), cpus).map_err(|e| {
                    KuroError::Cgroup(format!("Failed to write cpuset.cpus: {}", e))
                })?;
            }

            if let Some(mems) = cpuset.mems() {
                std::fs::write(self.path.join("cpuset.mems"), mems).map_err(|e| {
                    KuroError::Cgroup(format!("Failed to write cpuset.mems: {}", e))
                })?;
            }
        }

        Ok(())
    }

    /// Clean up cgroup
    pub fn destroy(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_dir(&self.path).map_err(|e| {
                KuroError::Cgroup(format!("Failed to remove cgroup directory: {}", e))
            })?;
        }

        Ok(())
    }

    /// Apply device filters
    pub fn apply_device_rules(cgroup_path: &Path, rules: &[LinuxDeviceCgroup]) -> Result<()> {
        for rule in rules {
            let allow_str = format!(
                "{} {}:{} {}",
                if rule.allow() { "a" } else { "b" },
                rule.major().map_or("".to_string(), |m| m.to_string()),
                rule.minor().map_or("".to_string(), |m| m.to_string()),
                rule.access().as_deref().unwrap_or("rwm")
            );

            let dev_file = cgroup_path.join("devices.allow");
            if dev_file.exists() {
                let _ = fs::write(dev_file, allow_str);
            }
        }

        Ok(())
    }
}
