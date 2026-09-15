use std::{
    fs::File,
    os::fd::AsFd,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{
    cgroups::v2::CgroupMgr,
    container::state::{ContainerState, ContainerStatus},
    error::{KuroError, Result},
    namespaces::{mount::MountMgr, netns::NetMgr, userns::UserMgr},
    sync::pipe::SyncPipe,
};
use nix::{
    sched::{CloneFlags, clone, setns},
    sys::signal::Signal,
    unistd::{Pid, sethostname},
};
use oci_spec::runtime::{LinuxNamespaceType, Spec};

// 1MB stack size for container child process
const STACK_SIZE: usize = 1024 * 1024;

pub struct CBuilder<'a> {
    pub container_id: String,
    pub bundle_path: PathBuf,
    pub spec: &'a Spec,
}

impl<'a> CBuilder<'a> {
    /// Setup new container builder
    pub fn new(container_id: String, bundle_path: PathBuf, spec: &'a Spec) -> Self {
        Self {
            container_id,
            bundle_path,
            spec,
        }
    }

    /// Create container
    pub fn create(&self) -> Result<Pid> {
        // Create sync pipes
        let parent_to_child = SyncPipe::new()?;
        let child_to_parent = SyncPipe::new()?;

        // Get clone flags
        let clone_flags = self.get_clone_flags()?;

        // Prepare child stack
        let mut stk = vec![0u8; STACK_SIZE];

        // Closure executed in child process
        let child_fn = Box::new(|| -> isize {
            match self.run_child_init(&child_to_parent, &parent_to_child) {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("[kuro-child] Error during initialization: {}", e);
                    1
                }
            }
        });

        // Clone the process into new namespaces
        let child_pid = unsafe {
            clone(
                child_fn,
                &mut stk,
                clone_flags,
                Some(Signal::SIGCHLD as i32),
            )
            .map_err(|e| KuroError::ExecFailed(format!("Failed to clone child process: {}", e)))?
        };

        println!(
            "[kuro-host] Spawned container init process with PID: {}",
            child_pid
        );

        // Setup user namespace mappings
        if let Some(linux) = self.spec.linux() {
            UserMgr::set_mappings(
                child_pid,
                linux.uid_mappings().as_deref(),
                linux.gid_mappings().as_deref(),
            )?;
        }

        // Setup cgroups
        let cmgr = CgroupMgr::new(&self.container_id)?;
        if let Some(linux) = self.spec.linux() {
            if let Some(rsrcs) = linux.resources() {
                cmgr.apply_limits(rsrcs)?;
            }
        }
        cmgr.add_proc(child_pid)?;

        // Network setup
        // NetMgr::setup_network(child_pid, None)?;

        // Update and save state (status = Created)
        let mut state = ContainerState::load(&self.container_id)?;
        state.status = ContainerStatus::Created;
        state.save()?;

        // [x]   Write UID/GID mappings
        // [x]   Create and add child_pid to cgroups-v2
        // [x]   Apply resource limits
        // [x]   Setup network interfaces in netns
        // [x]   Save container state (status = Created)
        // [x]   Network Manager implemented, but move calling from host to child process
        // TODO: createRuntime hooks

        // Signal child that host setup is done
        parent_to_child.send_signal()?;

        // Wait for child to ack rootfs + security setup
        child_to_parent.wait_for_signal()?;

        // [x]   Handle existing namespaces setting (setns() if path given)
        // -> Do this by looping through linux.namespaces()
        // -> For every ns, send typ() and path() to a fn setup_ns()
        // -> Inside setup_ns(), check the type of ns, attach/create ns, and call setup fns as needed
        // -> Handle PID namespaces with no path (ie CLONE_NEWPID) in host process, as unshare() fails

        Ok(child_pid)
    }

    /// Child container execution (Level 2)
    fn run_child_init(&self, child_to_parent: &SyncPipe, parent_to_child: &SyncPipe) -> Result<()> {
        // Wait for host to complete setup
        parent_to_child.wait_for_signal()?;

        // Handle all namespaces
        if let Some(linux) = self.spec.linux() {
            if let Some(namespaces) = linux.namespaces() {
                for ns in namespaces {
                    Self::setup_ns(ns.typ(), ns.path().as_ref())?;
                }
            }
        }

        // Set hostname
        if let Some(hostname) = self.spec.hostname() {
            sethostname(hostname).map_err(|e| {
                KuroError::Namespace(format!("Failed to set hostname '{}': {}", hostname, e))
            })?;
        }

        // Setup mounts, pivot_root, and masked/readonly paths
        MountMgr::setup_mount(self.spec, &self.container_id, &self.bundle_path)?;

        // [x]   Setup hostname
        // [x]   Mount filesystems and pivot_root
        // [x]   Masked and readonly paths
        // TODO: Apply capabilities, rlimits, env vars, no_new_privs
        // TODO: createContainer hooks

        // Signal parent that container setup is ready
        child_to_parent.send_signal()?;

        // TODO: Pause for 'kuro start' signal
        // [for now, we simulate the pause with a looped sleep]
        println!("[kuro-child] Container initialized and paused, ready to start");
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }

        Ok(())
    }

    /// Detect and attach different namespaces dynamically
    fn setup_ns(typ: LinuxNamespaceType, path: Option<&PathBuf>) -> Result<()> {
        // EXCLUDE UserNS: Mapping already compelete in host
        if typ == LinuxNamespaceType::User {
            return Ok(());
        }

        if let Some(path) = path {
            // Path provided; attach existing namespace
            let fd = File::open(&path).map_err(|e| {
                KuroError::Namespace(format!("Failed to open namespace path: {}", e))
            })?;
            if let Some(flag) = Self::get_clone_flag(typ) {
                setns(fd.as_fd(), flag).map_err(|e| {
                    KuroError::Namespace(format!(
                        "Failed to set namespace '{}' to path '{:?}': {}",
                        typ.to_string(),
                        path,
                        e
                    ))
                })?;
            } else {
                // Unsupported flags / namespace types (time)
                return Err(KuroError::Namespace(format!(
                    "Unsupported namespace type: {}",
                    typ.to_string()
                )));
            }
        } else {
            // Path not provided; create and setup new namespace
            Self::call_setup_fns(typ)?;
        }

        Ok(())
    }

    /// Get clone flag from namespace type
    fn get_clone_flag(typ: LinuxNamespaceType) -> Option<CloneFlags> {
        match typ {
            LinuxNamespaceType::Pid => Some(CloneFlags::CLONE_NEWPID),
            LinuxNamespaceType::Network => Some(CloneFlags::CLONE_NEWNET),
            LinuxNamespaceType::Ipc => Some(CloneFlags::CLONE_NEWIPC),
            LinuxNamespaceType::Uts => Some(CloneFlags::CLONE_NEWUTS),
            LinuxNamespaceType::Mount => Some(CloneFlags::CLONE_NEWNS),
            LinuxNamespaceType::Cgroup => Some(CloneFlags::CLONE_NEWCGROUP),
            LinuxNamespaceType::User => Some(CloneFlags::CLONE_NEWUSER),
            // LinuxNamespaceType::Time => libc::CLONE_NEWTIME as CloneFlags,
            _ => None,
        }
    }

    /// Call namespace setup methods dynamically
    fn call_setup_fns(typ: LinuxNamespaceType) -> Result<()> {
        match typ {
            LinuxNamespaceType::Network => NetMgr::setup_network(),
            LinuxNamespaceType::Mount
            | LinuxNamespaceType::Pid
            | LinuxNamespaceType::Uts
            | LinuxNamespaceType::Ipc
            | LinuxNamespaceType::Cgroup => return Ok(()),
            _ => Err(KuroError::Namespace(
                "Unsupported namespace type".to_string(),
            )),
        }
    }

    /// Get clone flags from spec
    fn get_clone_flags(&self) -> Result<CloneFlags> {
        let mut flags = CloneFlags::empty();

        if let Some(linux) = self.spec.linux() {
            if let Some(namespaces) = linux.namespaces() {
                for ns in namespaces {
                    // Only add clone flags for namespaces having no external path
                    if ns.path().is_none() {
                        if let Some(flag) = Self::get_clone_flag(ns.typ()) {
                            flags.insert(flag);
                        }
                    }
                }
            }
        }

        Ok(flags)
    }
}
