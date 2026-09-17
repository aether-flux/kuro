use std::{
    fs,
    path::{Path, PathBuf},
};

use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    unistd::{chdir, pivot_root},
};
use oci_spec::runtime::{Linux, Spec};

use crate::error::{KuroError, Result};

// [x]   Masked and readonly paths

pub struct MountMgr;

impl MountMgr {
    /// Setup new mount namespace
    pub fn setup_mount(spec: &Spec, container_id: &str, bundle: &PathBuf) -> Result<()> {
        Self::setup_overlayfs(&spec, container_id, bundle)?;
        Self::mount_fs(&spec)?;

        if let Some(linux) = spec.linux() {
            Self::set_masked(&linux)?;
            Self::set_readonly(&linux)?;
        }

        Ok(())
    }

    /// Setup overlayfs
    fn setup_overlayfs(spec: &Spec, container_id: &str, bundle: &PathBuf) -> Result<()> {
        // Define dir paths
        let base = bundle.join(container_id);
        let upper = base.join("upper");
        let work = base.join("work");
        let merged = base.join("merged");
        let lower = if let Some(rootfs) = spec.root().as_ref() {
            bundle.join(rootfs.path())
        } else {
            bundle.join("rootfs")
        };

        // Create the directories
        fs::create_dir_all(&upper)?;
        fs::create_dir_all(&merged)?;
        fs::create_dir_all(&work)?;

        // Make mount propagation PRIVATE to prevent mounts leaking to host
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(|e| KuroError::MountFailed {
            target: "/".to_string(),
            source: e,
        })?;

        // Mount OverlayFS
        let overlay_opts = format!(
            "lowerdir={},upperdir={},workdir={}",
            lower.display(),
            upper.display(),
            work.display()
        );
        mount(
            Some("overlay"),
            &merged,
            Some("overlay"),
            MsFlags::empty(),
            Some(overlay_opts.as_str()),
        )
        .map_err(|e| KuroError::MountFailed {
            target: "overlay".to_string(),
            source: e,
        })?;

        // pivot_root into isolated mount space
        Self::setup_pivot_root(&merged)?;

        Ok(())
    }

    /// Setup pivot_root
    fn setup_pivot_root(merged: &PathBuf) -> Result<()> {
        // Get absolute path of root
        let root = fs::canonicalize(&merged)?;
        chdir(&root)
            .map_err(|e| KuroError::Mount(format!("Failed to chdir to new root: {}", e)))?;

        // Bind-mount root onto itself (pre-requisite for pivot_root)
        mount(
            Some(&root),
            &root,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(|e| KuroError::MountFailed {
            target: root.to_string_lossy().to_string(),
            source: e,
        })?;

        // Handle old root safely
        let old_root = root.join("old_root");
        fs::create_dir_all(&old_root)?;
        // WARN: If bugs arise, change "." to &root
        pivot_root(&root, &old_root)
            .map_err(|e| KuroError::Mount(format!("Failed to pivot root: {}", e)))?;

        // Clean up old root
        chdir("/")
            .map_err(|e| KuroError::Mount(format!("Failed to change directory to '/': {}", e)))?;
        umount2("/old_root", MntFlags::MNT_DETACH)
            .map_err(|e| KuroError::Mount(format!("Failed to unmount /old_root: {}", e)))?;
        fs::remove_dir("/old_root")?;

        Ok(())
    }

    /// Mount filesystems given in config
    fn mount_fs(spec: &Spec) -> Result<()> {
        if let Some(mounts) = spec.mounts() {
            for mnt in mounts {
                // Ensure destination path
                let dest = mnt.destination();
                if !dest.exists() {
                    fs::create_dir_all(&dest)?;
                }

                // Parse mount options
                let opts = mnt.options().as_deref().unwrap_or(&[]);
                let (flags, data) = Self::parse_mount_options(opts);
                let data = if data.is_empty() {
                    None
                } else {
                    Some(data.as_str())
                };

                // Convert OCI spec to syscall types
                let source = mnt.source().as_deref();
                let fstype = mnt.typ().as_deref();

                mount(source, dest, fstype, flags, data).map_err(|e| KuroError::MountFailed {
                    target: dest.to_string_lossy().to_string(),
                    source: e,
                })?;
            }
        }

        Ok(())
    }

    /// Parse mount options in OCI spec
    fn parse_mount_options(options: &[String]) -> (MsFlags, String) {
        let mut flags = MsFlags::empty();
        let mut data_opts = Vec::new();

        for opt in options {
            match opt.as_str() {
                "ro" => flags.insert(MsFlags::MS_RDONLY),
                "nosuid" => flags.insert(MsFlags::MS_NOSUID),
                "nodev" => flags.insert(MsFlags::MS_NODEV),
                "noexec" => flags.insert(MsFlags::MS_NOEXEC),
                "bind" => flags.insert(MsFlags::MS_BIND),
                "rbind" => {
                    flags.insert(MsFlags::MS_BIND);
                    flags.insert(MsFlags::MS_REC);
                }
                "rec" => flags.insert(MsFlags::MS_REC),
                "remount" => flags.insert(MsFlags::MS_REMOUNT),
                "private" => flags.insert(MsFlags::MS_PRIVATE),
                "slave" => flags.insert(MsFlags::MS_SLAVE),
                "shared" => flags.insert(MsFlags::MS_SHARED),
                other => data_opts.push(other),
            }
        }

        let data = data_opts.join(",");

        (flags, data)
    }

    /// Set up readonly paths (RDONLY)
    fn set_readonly(linux: &Linux) -> Result<()> {
        if let Some(rdpaths) = linux.readonly_paths() {
            for path in rdpaths {
                // Mount as bind-mount
                mount(
                    Some(path.as_str()),
                    path.as_str(),
                    None::<&str>,
                    MsFlags::MS_BIND | MsFlags::MS_REC,
                    None::<&str>,
                )
                .map_err(|e| KuroError::MountFailed {
                    target: path.to_owned(),
                    source: e,
                })?;
                // Remount as read-only (RDONLY)
                mount(
                    None::<&str>,
                    path.as_str(),
                    None::<&str>,
                    MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY | MsFlags::MS_BIND | MsFlags::MS_REC,
                    None::<&str>,
                )
                .map_err(|e| KuroError::MountFailed {
                    target: path.to_owned(),
                    source: e,
                })?;
            }
        }

        Ok(())
    }

    /// Set up masked paths
    fn set_masked(linux: &Linux) -> Result<()> {
        if let Some(maskpaths) = linux.masked_paths() {
            for path in maskpaths {
                let path = Path::new(path);
                if !path.exists() {
                    continue;
                }

                if path.is_dir() {
                    // If path is a directory, mask it over an empty read-only tmpfs directory
                    mount(
                        Some("tmpfs"),
                        path,
                        Some("tmpfs"),
                        MsFlags::MS_RDONLY,
                        Some("mode=000"),
                    )
                    .map_err(|e| KuroError::MountFailed {
                        target: path.to_string_lossy().to_string(),
                        source: e,
                    })?;
                } else {
                    // If path is a file, bind-mount it over /dev/null
                    mount(
                        Some("/dev/null"),
                        path,
                        None::<&str>,
                        MsFlags::MS_BIND,
                        None::<&str>,
                    )
                    .map_err(|e| KuroError::MountFailed {
                        target: path.to_string_lossy().to_string(),
                        source: e,
                    })?;
                }
            }
        }

        Ok(())
    }
}
