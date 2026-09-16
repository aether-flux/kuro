use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use nix::{sys::stat::Mode, unistd::mkfifo};

use crate::error::{KuroError, Result};

pub struct ExecFifo {
    path: PathBuf,
}

impl ExecFifo {
    /// Create FIFO pipe
    pub fn create(container_dir: &Path) -> Result<Self> {
        let path = container_dir.join("exec.fifo");
        if path.exists() {
            fs::remove_file(&path)?;
        }

        // Create FIFO with read/write permissions for owner
        mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to create exec.fifo: {}", e)))?;

        Ok(Self { path })
    }

    /// Called inside run_child_init (PID 1 of container)
    pub fn wait_for_signal(&self) -> Result<()> {
        let mut fifo = File::open(&self.path).map_err(|e| {
            KuroError::SyncPipe(format!("Error opening file '{:?}': {}", &self.path, e))
        })?;
        let mut buf = [0u8; 1];
        fifo.read_exact(&mut buf).map_err(|e| {
            KuroError::SyncPipe(format!(
                "Error waiting to read from '{:?}': {}",
                &self.path, e
            ))
        })?;

        // After unblocking, remove/cleanup FIFO file
        let _ = fs::remove_file(&self.path);
        Ok(())
    }

    /// Called during 'kuro start' command
    pub fn signal_start(container_dir: &Path) -> Result<()> {
        let path = container_dir.join("exec.fifo");
        let mut file = OpenOptions::new().write(true).open(&path).map_err(|e| {
            KuroError::SyncPipe(format!("Error creating file '{:?}': {}", &path, e))
        })?;
        file.write_all(&[1u8]).map_err(|e| {
            KuroError::SyncPipe(format!("Error writing to file '{:?}': {}", &path, e))
        })?;

        Ok(())
    }
}
