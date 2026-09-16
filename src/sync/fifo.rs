use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use libc::O_NONBLOCK;
use nix::{sys::stat::Mode, unistd::mkfifo};

use crate::error::{KuroError, Result};

pub struct ExecFifo;

impl ExecFifo {
    /// Create FIFO path
    pub fn create_path(container_dir: &Path) -> PathBuf {
        container_dir.join("exec.fifo")
    }

    /// Create FIFO pipe
    pub fn init(container_dir: &Path) -> Result<PathBuf> {
        let path = Self::create_path(&container_dir);
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to create '{:?}': {}", &path, e)))?;
        Ok(path)
    }

    /// Open file while host /run is available
    pub fn open_for_read(fifo_path: &Path) -> Result<File> {
        // File::open(&fifo_path)
        //     .map_err(|e| KuroError::SyncPipe(format!("Failed to open '{:?}': {}", &fifo_path, e)))
        OpenOptions::new()
            .read(true)
            .custom_flags(O_NONBLOCK)
            .open(fifo_path)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to open '{:?}': {}", &fifo_path, e)))
    }

    /// Wait on already-opened file descriptor
    pub fn wait_for_start(fifo_file: File) -> Result<()> {
        // Clear O_NONBLOCK so read_exact() can now block
        use std::os::unix::io::AsRawFd;
        let fd = fifo_file.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags & !O_NONBLOCK);
        }

        let mut fifo_file = fifo_file;
        let mut buf = [0u8; 1];
        fifo_file
            .read_exact(&mut buf)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to read signal: {}", e)))?;
        Ok(())
    }

    /// Send signal to child to unblock
    pub fn signal_start(container_dir: &Path) -> Result<()> {
        let path = Self::create_path(&container_dir);
        let mut fifo = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to create '{:?}': {}", &path, e)))?;
        fifo.write_all(&[1u8])
            .map_err(|e| KuroError::SyncPipe(format!("Failed to send signal: {}", e)))?;

        Ok(())
    }
}
