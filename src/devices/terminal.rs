use std::{
    fs::{File, OpenOptions},
    os::{
        fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    path::Path,
};

use nix::{
    pty::{OpenptyResult, openpty},
    unistd::{dup2, dup2_stderr, dup2_stdin, dup2_stdout, setsid},
};

use crate::{
    error::{KuroError, Result},
    sync::console::ConsoleSocket,
};

pub struct TermMgr;

impl TermMgr {
    /// Setup container IO depending on whether terminal is interactive (PTY) or not
    pub fn setup_terminal(interactive: bool, console_stream: &Option<UnixStream>) -> Result<()> {
        if interactive {
            // Get stream
            let stream = console_stream
                .as_ref()
                .ok_or_else(|| KuroError::SyncPipe("Console socket not connected".to_string()))?;

            // Create PTY master/slave pair
            let OpenptyResult { master, slave } = openpty(None, None)
                .map_err(|e| KuroError::ExecFailed(format!("Failed to openpty: {}", e)))?;

            // Send master fd to host (kuro start)
            println!("terminal: sending fd");
            ConsoleSocket::send_fd(stream, master.as_raw_fd())?;
            println!("terminal: sent fd to socket");
            // drop(master); // receiver keeps it alive

            setsid().map_err(|e| KuroError::ExecFailed(format!("Failed to setsid: {}", e)))?;

            unsafe {
                // TIOSCSTTY sets slave fd as the controlling terminal for this proc
                if libc::ioctl(slave.as_raw_fd(), libc::TIOCSCTTY, 0) < 0 {
                    // if libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                    return Err(KuroError::ExecFailed(
                        "Failed to set controlling terminal (TIOCSCTTY)".to_string(),
                    ));
                }
            }

            // Duplicate slave to stdin(fd0) stdout(fd1) stderr(fd2)
            // unsafe {
            // let slave_fd = slave.as_fd();
            // dup2(slave_fd, &mut OwnedFd::from_raw_fd(0.as_raw_fd()))
            //     .map_err(|e| KuroError::ExecFailed(format!("dup2 stdin failed: {}", e)))?;
            // dup2(slave_fd, &mut OwnedFd::from_raw_fd(1.as_raw_fd()))
            //     .map_err(|e| KuroError::ExecFailed(format!("dup2 stdout failed: {}", e)))?;
            // dup2(slave_fd, &mut OwnedFd::from_raw_fd(2.as_raw_fd()))
            //     .map_err(|e| KuroError::ExecFailed(format!("dup2 stderr failed: {}", e)))?;
            dup2_stdin(&slave)
                .map_err(|e| KuroError::ExecFailed(format!("dup2 stdin failed: {}", e)))?;
            dup2_stdout(&slave)
                .map_err(|e| KuroError::ExecFailed(format!("dup2 stdout failed: {}", e)))?;
            dup2_stderr(&slave)
                .map_err(|e| KuroError::ExecFailed(format!("dup2 stderr failed: {}", e)))?;
            // for fd in 0..=2 {
            //     if libc::dup2(slave_fd, fd) < 0 {
            //         return Err(KuroError::ExecFailed(format!(
            //             "dup2 to fd {} failed: {}",
            //             fd,
            //             std::io::Error::last_os_error()
            //         )));
            //     }
            // }
            // }

            drop(slave);

            // Return master fd so host can relay it to CLI IO
            // let master_file = unsafe { File::from_raw_fd(master.into_raw_fd()) };
            // Ok(Some(master_file))
        } else {
            // Non-interactive; ensure standard stream descriptors are valid
            Self::ensure_std_descriptors()?;
            // Ok(None)
        }
        Ok(())
    }

    fn ensure_std_descriptors() -> Result<()> {
        for fd in 0..=2 {
            if unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1 {
                let dev_null = OpenOptions::new()
                    .read(fd == 0)
                    .write(fd != 0)
                    .open("/dev/null")
                    .map_err(|e| {
                        KuroError::ExecFailed(format!("Failed to open /dev/null: {}", e))
                    })?;
                // unsafe {
                //     dup2(dev_null.as_fd(), &mut OwnedFd::from_raw_fd(fd.as_raw_fd())).map_err(
                //         |e| KuroError::ExecFailed(format!("dup2 to /dev/null failed: {}", e)),
                //     )?;
                // }

                match fd {
                    0 => dup2_stdin(dev_null.as_fd()),
                    1 => dup2_stdout(dev_null.as_fd()),
                    _ => dup2_stderr(dev_null.as_fd()),
                }
                .map_err(|e| KuroError::ExecFailed(format!("dup2 to /dev/null failed: {}", e)))?;
            }
        }

        Ok(())
    }
}
