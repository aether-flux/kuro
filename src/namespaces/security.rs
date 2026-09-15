use std::{collections::HashSet, env};

use caps::{CapSet, Capability, CapsHashSet};
use nix::{
    sys::{
        prctl::set_no_new_privs,
        resource::{Resource, setrlimit},
    },
    unistd::{Gid, Uid, setgroups, setresgid, setresuid},
};
use oci_spec::runtime::{
    LinuxCapabilities, LinuxSeccomp, LinuxSeccompAction, PosixRlimit, Spec, User,
};
use syscallz::{Action, Context, Syscall};

use crate::error::{KuroError, Result};

// [x]   Use syscallz crate and finish the method set_seccomp()

pub struct SecMgr;

impl SecMgr {
    /// Setup all security features
    pub fn setup_security(spec: &Spec) -> Result<()> {
        if let Some(proc) = spec.process() {
            // Set env vars
            if let Some(vars) = proc.env() {
                Self::set_env_vars(vars)?;
            }

            // Set rlimits
            if let Some(rlimits) = proc.rlimits() {
                Self::set_rlimits(rlimits)?;
            }

            // Set no_new_privs
            if let Some(val) = proc.no_new_privileges() {
                Self::set_no_new_privs(val)?;
            }

            // Set capabilities
            if let Some(caps) = proc.capabilities() {
                Self::set_caps(caps)?;
            }
        }

        Ok(())
    }

    /// Setup capabilities
    fn set_caps(cap_spec: &LinuxCapabilities) -> Result<()> {
        // Helper to parse capability strings "CAP_SYS_ADMIN" or "SYS_ADMIN"
        let parse_caps =
            |caps_set: Option<&HashSet<oci_spec::runtime::Capability>>| -> CapsHashSet {
                caps_set
                    .map(|set| {
                        set.iter()
                            .filter_map(|c| {
                                let cap_str = c.to_string();
                                let name = cap_str.strip_prefix("CAP_").unwrap_or(&cap_str);
                                name.parse::<Capability>().ok()
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };

        // Bounding capabilities
        let bounding = parse_caps(cap_spec.bounding().as_ref());
        caps::set(None, CapSet::Bounding, &bounding).map_err(|e| {
            KuroError::Capability(format!("Error setting bounding capabilities: {}", e))
        })?;

        // Inheritable capabilities
        let inheritable = parse_caps(cap_spec.inheritable().as_ref());
        caps::set(None, CapSet::Inheritable, &inheritable).map_err(|e| {
            KuroError::Capability(format!("Error setting inheritable capabilities: {}", e))
        })?;

        // Permitted capabilities
        let permitted = parse_caps(cap_spec.permitted().as_ref());
        caps::set(None, CapSet::Permitted, &permitted).map_err(|e| {
            KuroError::Capability(format!("Error setting permitted capabilities: {}", e))
        })?;

        // Effective capabilities
        let effective = parse_caps(cap_spec.effective().as_ref());
        caps::set(None, CapSet::Effective, &effective).map_err(|e| {
            KuroError::Capability(format!("Error setting effective capabilities: {}", e))
        })?;

        // Ambient capabilities
        let ambient = parse_caps(cap_spec.ambient().as_ref());
        if !ambient.is_empty() {
            caps::set(None, CapSet::Ambient, &ambient).map_err(|e| {
                KuroError::Capability(format!("Error setting ambient capabilities: {}", e))
            })?;
        }

        Ok(())
    }

    /// Set rlimits
    fn set_rlimits(rlimits: &[PosixRlimit]) -> Result<()> {
        for limit in rlimits {
            let rsrc = match limit.typ().to_string().as_str() {
                "RLIMIT_NOFILE" => Resource::RLIMIT_NOFILE,
                "RLIMIT_NPROC" => Resource::RLIMIT_NPROC,
                "RLIMIT_MEMLOCK" => Resource::RLIMIT_MEMLOCK,
                "RLIMIT_CORE" => Resource::RLIMIT_CORE,
                "RLIMIT_STACK" => Resource::RLIMIT_STACK,
                "RLIMIT_AS" => Resource::RLIMIT_AS,
                "RLIMIT_CPU" => Resource::RLIMIT_CPU,
                "RLIMIT_FSIZE" => Resource::RLIMIT_FSIZE,
                "RLIMIT_DATA" => Resource::RLIMIT_DATA,
                "RLIMIT_LOCKS" => Resource::RLIMIT_LOCKS,
                "RLIMIT_MSGQUEUE" => Resource::RLIMIT_MSGQUEUE,
                "RLIMIT_NICE" => Resource::RLIMIT_NICE,
                "RLIMIT_RSS" => Resource::RLIMIT_RSS,
                "RLIMIT_RTPRIO" => Resource::RLIMIT_RTPRIO,
                "RLIMIT_RTTIME" => Resource::RLIMIT_RTTIME,
                "RLIMIT_SIGPENDING" => Resource::RLIMIT_SIGPENDING,
                other => {
                    eprintln!("Unknown rlimit type: {}", other);
                    continue;
                }
            };

            setrlimit(rsrc, limit.soft(), limit.hard()).map_err(|e| {
                KuroError::Rlimits(format!(
                    "Failed to set rlimit '{} {} {}': {}",
                    limit.typ().to_string(),
                    limit.soft(),
                    limit.hard(),
                    e
                ))
            })?;
        }

        Ok(())
    }

    /// Set no_new_privileges
    fn set_no_new_privs(val: bool) -> Result<()> {
        if val {
            set_no_new_privs()
                .map_err(|e| KuroError::NoNewPrivs(format!("Error setting no_new_privs: {}", e)))?;
        }

        Ok(())
    }

    /// Set environment variables
    fn set_env_vars(vars: &Vec<String>) -> Result<()> {
        for var in vars {
            if let Some((key, val)) = var.split_once('=') {
                unsafe {
                    env::set_var(key, val);
                }
            }
        }

        Ok(())
    }

    /// Set user/group
    fn set_user(user: &User) -> Result<()> {
        if let Some(gids) = user.additional_gids() {
            let gids: Vec<Gid> = gids.iter().map(|&g| Gid::from_raw(g)).collect();
            setgroups(&gids).map_err(|e| KuroError::Userns(format!("Failed setgroups: {}", e)))?;
        }

        let gid = Gid::from_raw(user.gid());
        let uid = Uid::from_raw(user.uid());

        setresgid(gid, gid, gid)
            .map_err(|e| KuroError::Userns(format!("Failed setresgid: {}", e)))?;
        setresuid(uid, uid, uid)
            .map_err(|e| KuroError::Userns(format!("Failed setresuid: {}", e)))?;

        Ok(())
    }

    /// Apply Seccomp filtering
    pub fn apply_seccomp(seccomp_spec: &LinuxSeccomp) -> Result<()> {
        let default_action = Self::map_seccomp(seccomp_spec.default_action())?;

        // Initialize syscallz with default action
        let mut ctx = Context::init_with_action(default_action).map_err(|e| {
            KuroError::Seccomp(format!("Failed to initialize seccomp context: {}", e))
        })?;

        // Load syscall rules from config
        if let Some(syscall_specs) = seccomp_spec.syscalls() {
            for spec in syscall_specs {
                let action = Self::map_seccomp(spec.action())?;

                for name in spec.names() {
                    if let Some(syscall) = Syscall::from_name(name) {
                        ctx.set_action_for_syscall(action, syscall).map_err(|e| {
                            KuroError::Seccomp(format!("Failed rule for '{}': {}", name, e))
                        })?;
                    } else {
                        // Skip unknown syscalls
                        eprintln!("WARN: Unknown syscall name in config: {}", name);
                    }
                }
            }
        }

        ctx.load().map_err(|e| {
            KuroError::Seccomp(format!("Failed to load seccomp filter into kernel: {}", e))
        })?;

        Ok(())
    }

    /// Map seccomp action to syscallz Action
    fn map_seccomp(action: LinuxSeccompAction) -> Result<Action> {
        let res = match action {
            LinuxSeccompAction::ScmpActAllow => Action::Allow,
            LinuxSeccompAction::ScmpActErrno => Action::Errno(1),
            LinuxSeccompAction::ScmpActKill => Action::KillThread,
            LinuxSeccompAction::ScmpActKillProcess => Action::KillProcess,
            LinuxSeccompAction::ScmpActLog => Action::Allow,
            LinuxSeccompAction::ScmpActTrap => Action::Trap,
            _ => {
                return Err(KuroError::Seccomp(format!(
                    "Unsupported seccomp action: {:?}",
                    action
                )));
            }
        };

        Ok(res)
    }
}
