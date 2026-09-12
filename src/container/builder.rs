use std::{path::PathBuf, time::Duration};

use crate::{
    error::{KuroError, Result},
    sync::pipe::SyncPipe,
};
use nix::{
    sched::{CloneFlags, clone},
    sys::signal::Signal,
    unistd::Pid,
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

        // TODO: Write UID/GID mappings
        // TODO: Create and add child_pid to cgroups-v2
        // TODO: Apply resource limits
        // TODO: Setup network interfaces in netns
        // TODO: Save container state (status = Created)
        // TODO: createRuntime hooks

        // Signal child that host setup is done
        parent_to_child.send_signal()?;

        // Wait for child to ack rootfs + security setup
        child_to_parent.wait_for_signal()?;

        Ok(child_pid)
    }

    /// Child container execution (Level 2)
    fn run_child_init(&self, child_to_parent: &SyncPipe, parent_to_child: &SyncPipe) -> Result<()> {
        // Wait for host to complete setup
        parent_to_child.wait_for_signal()?;

        // TODO: Setup hostname
        // TODO: Mount filesystems and pivot_root
        // TODO: Apply capabilities, rlimits, env vars, no_new_privs

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

    /// Get clone flags from spec
    fn get_clone_flags(&self) -> Result<CloneFlags> {
        let mut flags = CloneFlags::empty();

        if let Some(linux) = self.spec.linux() {
            if let Some(namespaces) = linux.namespaces() {
                for ns in namespaces {
                    match ns.typ() {
                        LinuxNamespaceType::Pid => flags.insert(CloneFlags::CLONE_NEWPID),
                        LinuxNamespaceType::Network => flags.insert(CloneFlags::CLONE_NEWNET),
                        LinuxNamespaceType::Ipc => flags.insert(CloneFlags::CLONE_NEWIPC),
                        LinuxNamespaceType::Uts => flags.insert(CloneFlags::CLONE_NEWUTS),
                        LinuxNamespaceType::Mount => flags.insert(CloneFlags::CLONE_NEWNS),
                        LinuxNamespaceType::Cgroup => flags.insert(CloneFlags::CLONE_NEWCGROUP),
                        LinuxNamespaceType::User => flags.insert(CloneFlags::CLONE_NEWUSER),
                        LinuxNamespaceType::Time => {}
                    }
                }
            }
        }

        Ok(flags)
    }
}
