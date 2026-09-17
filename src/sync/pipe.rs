use std::os::fd::{AsFd, OwnedFd};

use nix::{
    fcntl::OFlag,
    unistd::{pipe2, read, write},
};

use crate::error::{KuroError, Result};

pub enum SyncResult {
    Ready,
    Error(String),
    Eof,
}

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

    /// Send success signal (byte 1)
    pub fn send_ready(&self) -> Result<()> {
        let buf = [1u8];
        write(self.write_fd.as_fd(), &buf)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to send ready signal: {}", e)))?;

        Ok(())
    }

    /// Send error signal (byte 2 followed by 4-length header and UTF-8 string)
    pub fn send_err(&self, emsg: &str) -> Result<()> {
        let msg_bytes = emsg.as_bytes();
        let mlen = msg_bytes.len() as u32;

        let mut packet = Vec::with_capacity(1 + 4 + msg_bytes.len());
        packet.push(2u8);
        packet.extend_from_slice(&mlen.to_be_bytes());
        packet.extend_from_slice(msg_bytes);

        write(self.write_fd.as_fd(), &packet)
            .map_err(|e| KuroError::SyncPipe(format!("Failed to send error signal: {}", e)))?;

        Ok(())
    }

    /// Wait until a 1-byte signal is received
    pub fn wait_for_signal(&self) -> Result<SyncResult> {
        let mut flag_buf = [0u8; 1];
        let bytes_read = match read(self.read_fd.as_fd(), &mut flag_buf) {
            Ok(n) => n,
            Err(e) => {
                return Err(KuroError::SyncPipe(format!(
                    "Failed to read sync signal: {}",
                    e
                )));
            }
        };

        if bytes_read == 0 {
            return Ok(SyncResult::Eof);
        }

        match flag_buf[0] {
            1 => Ok(SyncResult::Ready),
            2 => {
                let mut len_buf = [0u8; 4];
                read_exact_fd(self.read_fd.as_fd(), &mut len_buf)?;
                let mlen = u32::from_be_bytes(len_buf) as usize;

                let mut msg_bytes = vec![0u8; mlen];
                read_exact_fd(self.read_fd.as_fd(), &mut msg_bytes)?;

                let emsg = String::from_utf8_lossy(&msg_bytes).to_string();
                Ok(SyncResult::Error(emsg))
            }
            unknown => Err(KuroError::SyncPipe(format!(
                "Received invalid status code from sync pipe: {}",
                unknown
            ))),
        }
    }
}

fn read_exact_fd(fd: std::os::fd::BorrowedFd, mut buf: &mut [u8]) -> Result<()> {
    while !buf.is_empty() {
        match read(fd, buf) {
            Ok(0) => {
                return Err(KuroError::SyncPipe(
                    "Unexpected EOF while reading error payload from sync pipe".to_string(),
                ));
            }
            Ok(n) => {
                let tmp = buf;
                buf = &mut tmp[n..];
            }
            Err(e) => {
                return Err(KuroError::SyncPipe(format!(
                    "Failed to read from pipe: {}",
                    e
                )));
            }
        }
    }

    Ok(())
}
