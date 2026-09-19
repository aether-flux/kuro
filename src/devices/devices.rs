use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use libc::dev_t;
use nix::{
    mount::{MsFlags, mount},
    sys::stat::{Mode, SFlag, mknod},
    unistd::{Gid, Uid, chown},
};
use oci_spec::runtime::{LinuxDevice, LinuxDeviceType};

use crate::error::{KuroError, Result};

pub struct DevMgr;

impl DevMgr {
    /// Default OCI container devices required by spec
    fn default_devices() -> Vec<LinuxDevice> {
        vec![
            Self::make_dev("/dev/null", LinuxDeviceType::C, 1, 3, 0o666),
            Self::make_dev("/dev/zero", LinuxDeviceType::C, 1, 5, 0o666),
            Self::make_dev("/dev/full", LinuxDeviceType::C, 1, 7, 0o666),
            Self::make_dev("/dev/random", LinuxDeviceType::C, 1, 8, 0o666),
            Self::make_dev("/dev/urandom", LinuxDeviceType::C, 1, 9, 0o666),
            Self::make_dev("/dev/tty", LinuxDeviceType::C, 5, 0, 0o666),
            Self::make_dev("/dev/ptmx", LinuxDeviceType::C, 5, 2, 0o666),
        ]
    }

    fn make_dev(
        path: &str,
        dev_type: LinuxDeviceType,
        major: i64,
        minor: i64,
        file_mode: u32,
    ) -> LinuxDevice {
        let mut dev = LinuxDevice::default();
        dev.set_path(PathBuf::from(path));
        dev.set_typ(dev_type);
        dev.set_major(major);
        dev.set_minor(minor);
        dev.set_file_mode(Some(file_mode));

        dev
    }

    /// Create missing device nodes inside container rootfs
    pub fn create_devices(dev_spec: Option<&[LinuxDevice]>) -> Result<()> {
        let mut all_dev = Self::default_devices();
        if let Some(extra) = dev_spec {
            all_dev.extend_from_slice(extra);
        }

        for dev in all_dev {
            let path = dev.path();
            if path.exists() {
                let _ = fs::remove_file(path);
            }

            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let sflag = match dev.typ() {
                LinuxDeviceType::C => SFlag::S_IFCHR,
                LinuxDeviceType::B => SFlag::S_IFBLK,
                LinuxDeviceType::P => SFlag::S_IFIFO,
                _ => SFlag::S_IFCHR,
            };

            let mode = Mode::from_bits_truncate(dev.file_mode().unwrap_or(0o600));
            let rawdev = makedev(dev.major() as u64, dev.minor() as u64);

            // Bind mount or mknod
            if let Err(e) = mknod(path, sflag, mode, rawdev) {
                if e == nix::errno::Errno::EPERM {
                    Self::bind_dev_from_host(path)?;
                } else {
                    return Err(KuroError::ExecFailed(format!(
                        "Failed to mknod device {:?}: {}",
                        path, e
                    )));
                }
            }

            // Apply UID/GID if specified
            if dev.uid().is_some() || dev.gid().is_some() {
                let uid = dev.uid().map(Uid::from_raw);
                let gid = dev.gid().map(Gid::from_raw);
                let _ = chown(path, uid, gid);
            }
        }

        // Setup standard symlinks
        Self::setup_dev_symlinks()?;

        Ok(())
    }

    /// Setup standard symlinks
    fn setup_dev_symlinks() -> Result<()> {
        let symlinks = [
            ("/proc/self/fd", "/dev/fd"),
            ("/proc/self/fd/0", "/dev/stdin"),
            ("/proc/self/fd/1", "/dev/stdout"),
            ("/proc/self/fd/2", "/dev/stderr"),
            ("/dev/pts/ptmx", "/dev/ptmx"),
        ];

        for (target, link) in symlinks {
            let link_path = Path::new(link);
            if !link_path.exists() {
                let _ = std::os::unix::fs::symlink(target, link_path);
            }
        }

        Ok(())
    }

    /// Bind mount (fallback)
    fn bind_dev_from_host(path: &Path) -> Result<()> {
        let _ = OpenOptions::new().write(true).create(true).open(path);
        mount(
            Some(path),
            path,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(|e| KuroError::MountFailed {
            target: path.to_string_lossy().to_string(),
            source: e,
        })?;

        Ok(())
    }
}

fn makedev(major: u64, minor: u64) -> dev_t {
    libc::makedev(major as u32, minor as u32)
}
