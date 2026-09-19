use std::{
    fs,
    io::{IoSlice, IoSliceMut},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::Path,
};

use nix::sys::socket::{ControlMessage, ControlMessageOwned, MsgFlags, recvmsg, sendmsg};

use crate::error::{KuroError, Result};

pub struct ConsoleSocket;

impl ConsoleSocket {
    /// Host: bind a fresh socket
    pub fn listen(path: &Path) -> Result<UnixListener> {
        let _ = fs::remove_file(path);
        UnixListener::bind(path).map_err(|e| {
            KuroError::ExecFailed(format!("Failed to bind console socket {:?}: {}", path, e))
        })
    }

    /// Host: accept one connection only and receive master PTY fd
    pub fn recv_fd(listener: &UnixListener) -> Result<RawFd> {
        let (stream, _) = listener.accept().map_err(|e| {
            KuroError::ExecFailed(format!("Failed to accept console connection: {}", e))
        })?;
        let raw_fd = stream.as_raw_fd();

        let mut byte = [0u8; 1];
        let mut iov = [IoSliceMut::new(&mut byte)];
        let mut cmsg_buffer = nix::cmsg_space!([RawFd; 1]);

        let msg = recvmsg::<()>(raw_fd, &mut iov, Some(&mut cmsg_buffer), MsgFlags::empty())
            .map_err(|e| {
                KuroError::ExecFailed(format!("recvmsg on console socket failed: {}", e))
            })?;

        for cmsg in msg
            .cmsgs()
            .map_err(|e| KuroError::ExecFailed(format!("Failed to read cmsgs: {}", e)))?
        {
            if let ControlMessageOwned::ScmRights(fds) = cmsg {
                if let Some(fd) = fds.into_iter().next() {
                    return Ok(fd);
                }
            }
        }

        Err(KuroError::ExecFailed(
            "No fd received over console socket".to_string(),
        ))
    }

    /// Child: connect to unix stream
    pub fn connect(path: &Path) -> Result<UnixStream> {
        UnixStream::connect(path).map_err(|e| {
            KuroError::ExecFailed(format!(
                "Failed to connect to console socket {:?}: {}",
                path, e
            ))
        })
    }

    /// Child: send master fd
    pub fn send_fd(stream: &UnixStream, fd: RawFd) -> Result<()> {
        let raw_fd = stream.as_raw_fd();

        let byte = [b'c'];
        let iov = [IoSlice::new(&byte)];
        let fds = [fd];
        let cmsgs = [ControlMessage::ScmRights(&fds)];

        sendmsg::<()>(raw_fd, &iov, &cmsgs, MsgFlags::empty(), None).map_err(|e| {
            KuroError::ExecFailed(format!("sendmsg on console socket failed: {}", e))
        })?;

        Ok(())
    }
}
