use std::os::fd::{AsFd, OwnedFd};

use nix::{
    fcntl::OFlag,
    unistd::{pipe2, read, write},
};

use crate::error::{KuroError, Result};

/// Bi-directional sync pipe for host <-> container
pub struct SyncPipe {
    read_fd: OwnedFd,
    write_fd: OwnedFd,
}

impl SyncPipe {
    /// Create new pipe with O_CLOEXEC (automatically closed on execve)
    pub fn new() -> Result<Self> {
        let (read_fd, write_fd) = pipe2(OFlag::O_CLOEXEC)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to create pipe: {}", e)))?;

        Ok(Self { read_fd, write_fd })
    }

    /// Send a 1-byte signal
    pub fn send_signal(&self) -> Result<()> {
        let buf = [1u8];
        write(self.write_fd.as_fd(), &buf)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to send sync signal: {}", e)))?;

        Ok(())
    }

    /// Wait until a 1-byte signal is received
    pub fn wait_for_signal(&self) -> Result<()> {
        let mut buf = [0u8; 1];
        let bytes_read = read(self.read_fd.as_fd(), &mut buf)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to read sync signal: {}", e)))?;

        if bytes_read == 0 {
            return Err(KuroError::SyncPipe(
                "Sync pipe closed unexpectedly before receiving signal".to_string(),
            ));
        }

        Ok(())
    }
}
