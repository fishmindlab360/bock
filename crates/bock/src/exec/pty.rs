//! PTY (pseudo-terminal) handling.
//!
//! This module provides utilities for allocating and managing pseudo-terminals
//! for container console support.

#![allow(unsafe_code)]

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::path::PathBuf;

use bock_common::BockResult;

/// PTY file descriptor pair.
pub struct PtyPair {
    /// Master file descriptor.
    master: OwnedFd,
    /// Slave path.
    slave_path: PathBuf,
}

impl PtyPair {
    /// Allocate a new PTY pair.
    #[cfg(target_os = "linux")]
    pub fn new() -> BockResult<Self> {
        use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};

        tracing::debug!("Allocating PTY");

        // Open master PTY
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to open PTY master: {}", e),
            }
        })?;

        // Grant access to slave
        grantpt(&master).map_err(|e| bock_common::BockError::Internal {
            message: format!("grantpt failed: {}", e),
        })?;

        // Unlock slave
        unlockpt(&master).map_err(|e| bock_common::BockError::Internal {
            message: format!("unlockpt failed: {}", e),
        })?;

        // Get slave path
        let slave_path =
            ptsname(&master, Vec::new()).map_err(|e| bock_common::BockError::Internal {
                message: format!("ptsname failed: {}", e),
            })?;

        let slave_path = PathBuf::from(OsString::from_vec(slave_path.into_bytes()));

        tracing::debug!(slave_path = %slave_path.display(), "PTY allocated");

        Ok(Self { master, slave_path })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new() -> BockResult<Self> {
        Err(bock_common::BockError::Unsupported {
            feature: "pty".to_string(),
        })
    }

    /// Get the master file descriptor.
    pub fn master_fd(&self) -> RawFd {
        self.master.as_raw_fd()
    }

    /// Get the path to the slave PTY device.
    pub fn slave_path(&self) -> &PathBuf {
        &self.slave_path
    }

    /// Open the slave PTY.
    #[cfg(target_os = "linux")]
    pub fn open_slave(&self) -> BockResult<OwnedFd> {
        use rustix::fs::{Mode, OFlags, open};

        let fd = open(
            &self.slave_path,
            OFlags::RDWR | OFlags::NOCTTY,
            Mode::empty(),
        )
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to open PTY slave: {}", e),
        })?;

        Ok(fd)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open_slave(&self) -> BockResult<OwnedFd> {
        Err(bock_common::BockError::Unsupported {
            feature: "pty".to_string(),
        })
    }

    /// Set the terminal window size.
    #[cfg(target_os = "linux")]
    pub fn set_size(&self, rows: u16, cols: u16) -> BockResult<()> {
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let result = unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) };

        if result != 0 {
            return Err(bock_common::BockError::Internal {
                message: format!(
                    "Failed to set PTY size: {}",
                    std::io::Error::last_os_error()
                ),
            });
        }

        tracing::debug!(rows, cols, "PTY size set");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn set_size(&self, _rows: u16, _cols: u16) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "pty".to_string(),
        })
    }

    /// Get the current terminal window size.
    #[cfg(target_os = "linux")]
    pub fn get_size(&self) -> BockResult<(u16, u16)> {
        let mut winsize = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let result =
            unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCGWINSZ, &mut winsize) };

        if result != 0 {
            return Err(bock_common::BockError::Internal {
                message: format!(
                    "Failed to get PTY size: {}",
                    std::io::Error::last_os_error()
                ),
            });
        }

        Ok((winsize.ws_row, winsize.ws_col))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_size(&self) -> BockResult<(u16, u16)> {
        Err(bock_common::BockError::Unsupported {
            feature: "pty".to_string(),
        })
    }

    /// Make the slave PTY the controlling terminal for the current process.
    #[cfg(target_os = "linux")]
    pub fn make_controlling_terminal(&self) -> BockResult<()> {
        let slave = self.open_slave()?;

        // Create a new session
        unsafe {
            if libc::setsid() < 0 {
                return Err(bock_common::BockError::Internal {
                    message: format!("setsid failed: {}", std::io::Error::last_os_error()),
                });
            }

            // Set controlling terminal
            if libc::ioctl(slave.as_raw_fd(), libc::TIOCSCTTY, 0) < 0 {
                return Err(bock_common::BockError::Internal {
                    message: format!("TIOCSCTTY failed: {}", std::io::Error::last_os_error()),
                });
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn make_controlling_terminal(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "pty".to_string(),
        })
    }

    /// Setup the slave PTY as stdin/stdout/stderr.
    #[cfg(target_os = "linux")]
    pub fn setup_stdio(&self) -> BockResult<()> {
        let slave = self.open_slave()?;
        let fd = slave.as_raw_fd();

        unsafe {
            // Duplicate slave to stdin, stdout, stderr
            if libc::dup2(fd, libc::STDIN_FILENO) < 0 {
                return Err(bock_common::BockError::Internal {
                    message: "Failed to dup2 stdin".to_string(),
                });
            }
            if libc::dup2(fd, libc::STDOUT_FILENO) < 0 {
                return Err(bock_common::BockError::Internal {
                    message: "Failed to dup2 stdout".to_string(),
                });
            }
            if libc::dup2(fd, libc::STDERR_FILENO) < 0 {
                return Err(bock_common::BockError::Internal {
                    message: "Failed to dup2 stderr".to_string(),
                });
            }
        }

        tracing::debug!("PTY stdio configured");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn setup_stdio(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "pty".to_string(),
        })
    }
}

impl std::fmt::Debug for PtyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyPair")
            .field("master_fd", &self.master.as_raw_fd())
            .field("slave_path", &self.slave_path)
            .finish()
    }
}

/// Legacy function for backwards compatibility.
#[deprecated(note = "Use PtyPair::new() instead")]
pub fn allocate_pty() -> BockResult<PtyPair> {
    PtyPair::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_pty_allocation() {
        let pty = PtyPair::new();
        assert!(
            pty.is_ok(),
            "PTY allocation should succeed: {:?}",
            pty.err()
        );

        let pty = pty.unwrap();
        assert!(pty.slave_path().exists(), "Slave path should exist");
        assert!(pty.master_fd() >= 0, "Master fd should be valid");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_pty_size() {
        let pty = PtyPair::new().unwrap();
        assert!(pty.set_size(24, 80).is_ok());

        let (rows, cols) = pty.get_size().unwrap();
        assert_eq!(rows, 24);
        assert_eq!(cols, 80);
    }
}
