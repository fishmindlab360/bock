//! Console socket for remote terminal attach.
//!
//! This module provides utilities for creating and managing Unix domain sockets
//! that allow remote clients to attach to container terminals.

#![allow(unsafe_code)]

use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use bock_common::BockResult;

/// Console socket for terminal attach/detach.
pub struct ConsoleSocket {
    /// Path to the socket.
    path: PathBuf,
    /// Unix listener.
    listener: UnixListener,
}

impl ConsoleSocket {
    /// Create a new console socket at the given path.
    pub fn new(path: &Path) -> BockResult<Self> {
        // Remove existing socket if present
        if path.exists() {
            std::fs::remove_file(path)?;
        }

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(path).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to bind console socket: {}", e),
        })?;

        tracing::info!(path = %path.display(), "Console socket created");

        Ok(Self {
            path: path.to_path_buf(),
            listener,
        })
    }

    /// Accept a connection on the socket.
    pub fn accept(&self) -> BockResult<(UnixStream, std::os::unix::net::SocketAddr)> {
        let (stream, addr) =
            self.listener
                .accept()
                .map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to accept console connection: {}", e),
                })?;

        tracing::debug!("Console client connected");
        Ok((stream, addr))
    }

    /// Set the socket to non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> BockResult<()> {
        self.listener.set_nonblocking(nonblocking).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to set non-blocking: {}", e),
            }
        })?;
        Ok(())
    }

    /// Get the raw file descriptor.
    pub fn as_raw_fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }

    /// Get the socket path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ConsoleSocket {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Console client for attaching to containers.
pub struct ConsoleClient {
    /// Connected stream.
    stream: UnixStream,
}

impl ConsoleClient {
    /// Connect to a console socket.
    pub fn connect(path: &Path) -> BockResult<Self> {
        let stream = UnixStream::connect(path).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to connect to console socket: {}", e),
        })?;

        tracing::debug!(path = %path.display(), "Connected to console");

        Ok(Self { stream })
    }

    /// Get the underlying stream.
    pub fn stream(&self) -> &UnixStream {
        &self.stream
    }

    /// Get mutable access to the stream.
    pub fn stream_mut(&mut self) -> &mut UnixStream {
        &mut self.stream
    }

    /// Set non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> BockResult<()> {
        self.stream
            .set_nonblocking(nonblocking)
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to set non-blocking: {}", e),
            })?;
        Ok(())
    }
}

/// Receive a PTY master file descriptor over Unix socket.
#[cfg(target_os = "linux")]
pub fn recv_pty_master(stream: &UnixStream) -> BockResult<RawFd> {
    let mut buf = [0u8; 1];
    let mut fds = [0i32; 1];

    let n = recv_fd(stream.as_raw_fd(), &mut buf, &mut fds)?;

    if n == 0 || fds[0] < 0 {
        return Err(bock_common::BockError::Internal {
            message: "No PTY master received".to_string(),
        });
    }

    tracing::debug!(fd = fds[0], "Received PTY master FD");
    Ok(fds[0])
}

#[cfg(not(target_os = "linux"))]
pub fn recv_pty_master(_stream: &UnixStream) -> BockResult<RawFd> {
    Err(bock_common::BockError::Unsupported {
        feature: "SCM_RIGHTS".to_string(),
    })
}

/// Send a PTY master file descriptor over Unix socket.
#[cfg(target_os = "linux")]
pub fn send_pty_master(stream: &UnixStream, fd: RawFd) -> BockResult<()> {
    let buf = [0u8; 1];

    send_fd(stream.as_raw_fd(), &buf, fd)?;

    tracing::debug!(fd, "Sent PTY master FD");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn send_pty_master(_stream: &UnixStream, _fd: RawFd) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "SCM_RIGHTS".to_string(),
    })
}

/// Low-level function to receive a file descriptor over Unix socket.
#[cfg(target_os = "linux")]
fn recv_fd(sock_fd: RawFd, buf: &mut [u8], fds: &mut [i32]) -> BockResult<usize> {
    use std::mem;
    use std::ptr;

    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };

    let mut cmsg_buf = [0u8; 64];

    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buf.as_mut_ptr() as *mut libc::c_void;
    msg.msg_controllen = cmsg_buf.len();

    let n = unsafe { libc::recvmsg(sock_fd, &mut msg, 0) };

    if n < 0 {
        return Err(bock_common::BockError::Internal {
            message: format!("recvmsg failed: {}", std::io::Error::last_os_error()),
        });
    }

    // Extract file descriptor from control message
    let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    if !cmsg.is_null() {
        let cmsg_ref = unsafe { &*cmsg };
        if cmsg_ref.cmsg_level == libc::SOL_SOCKET && cmsg_ref.cmsg_type == libc::SCM_RIGHTS {
            let data = unsafe { libc::CMSG_DATA(cmsg) };
            if !fds.is_empty() {
                fds[0] = unsafe { ptr::read(data as *const i32) };
            }
        }
    }

    Ok(n as usize)
}

/// Low-level function to send a file descriptor over Unix socket.
#[cfg(target_os = "linux")]
fn send_fd(sock_fd: RawFd, buf: &[u8], fd: RawFd) -> BockResult<()> {
    use std::mem;
    use std::ptr;

    let mut iov = libc::iovec {
        iov_base: buf.as_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };

    let mut cmsg_buf = [0u8; 64];

    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buf.as_mut_ptr() as *mut libc::c_void;
    msg.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as u32) as usize };

    let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    if !cmsg.is_null() {
        unsafe {
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            (*cmsg).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as u32) as usize;
            ptr::write(libc::CMSG_DATA(cmsg) as *mut RawFd, fd);
        }
    }

    let n = unsafe { libc::sendmsg(sock_fd, &msg, 0) };

    if n < 0 {
        return Err(bock_common::BockError::Internal {
            message: format!("sendmsg failed: {}", std::io::Error::last_os_error()),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_socket_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let socket_path = temp_dir.path().join("console.sock");

        let socket = ConsoleSocket::new(&socket_path);
        assert!(socket.is_ok(), "Socket creation should succeed");

        let socket = socket.unwrap();
        assert!(socket_path.exists(), "Socket file should exist");
        assert_eq!(socket.path(), socket_path);
    }

    #[test]
    fn test_console_client_connect() {
        let temp_dir = tempfile::tempdir().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let server = ConsoleSocket::new(&socket_path).unwrap();

        // Spawn thread to accept connection
        let path = socket_path.clone();
        let handle = std::thread::spawn(move || ConsoleClient::connect(&path));

        // Accept can block, so we set non-blocking for test
        server.set_nonblocking(true).ok();

        // Give client time to connect
        std::thread::sleep(std::time::Duration::from_millis(50));

        let client = handle.join().unwrap();
        assert!(client.is_ok(), "Client should connect");
    }
}
