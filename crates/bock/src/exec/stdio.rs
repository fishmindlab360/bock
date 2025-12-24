//! Standard I/O handling for containers.
//!
//! This module provides attach/detach modes for container I/O streams,
//! allowing containers to run in foreground (attached) or background (detached) modes.
#![allow(unsafe_code)]
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bock_common::BockResult;

/// Container stdio mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioMode {
    /// Attached mode - container I/O connected to terminal.
    Attached,
    /// Detached mode - container runs in background.
    Detached,
    /// Stream mode - I/O redirected to streams.
    Stream,
}

impl Default for StdioMode {
    fn default() -> Self {
        Self::Attached
    }
}

/// Container stdio configuration.
#[derive(Debug, Clone)]
pub struct StdioConfig {
    /// Stdio mode.
    pub mode: StdioMode,
    /// Attach stdin.
    pub attach_stdin: bool,
    /// Attach stdout.
    pub attach_stdout: bool,
    /// Attach stderr.
    pub attach_stderr: bool,
    /// Open stdin.
    pub open_stdin: bool,
    /// Allocate TTY.
    pub tty: bool,
}

impl Default for StdioConfig {
    fn default() -> Self {
        Self {
            mode: StdioMode::Attached,
            attach_stdin: true,
            attach_stdout: true,
            attach_stderr: true,
            open_stdin: true,
            tty: false,
        }
    }
}

impl StdioConfig {
    /// Create detached configuration.
    pub fn detached() -> Self {
        Self {
            mode: StdioMode::Detached,
            attach_stdin: false,
            attach_stdout: false,
            attach_stderr: false,
            open_stdin: false,
            tty: false,
        }
    }

    /// Create attached with TTY configuration.
    pub fn with_tty() -> Self {
        Self {
            mode: StdioMode::Attached,
            attach_stdin: true,
            attach_stdout: true,
            attach_stderr: true,
            open_stdin: true,
            tty: true,
        }
    }
}

/// Stdio handler for container I/O.
pub struct StdioHandler {
    /// Configuration.
    config: StdioConfig,
    /// Stop flag.
    stop: Arc<AtomicBool>,
}

impl StdioHandler {
    /// Create a new stdio handler.
    pub fn new(config: StdioConfig) -> Self {
        Self {
            config,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if in attached mode.
    pub fn is_attached(&self) -> bool {
        self.config.mode == StdioMode::Attached
    }

    /// Check if TTY is enabled.
    pub fn has_tty(&self) -> bool {
        self.config.tty
    }

    /// Get stop flag for shutdown signaling.
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop)
    }

    /// Signal stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Check if stopped.
    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Forward stdin to container.
    #[cfg(target_os = "linux")]
    pub fn forward_stdin<W: Write>(&self, mut writer: W) -> BockResult<()> {
        if !self.config.attach_stdin {
            return Ok(());
        }

        let stdin = std::io::stdin();
        let mut stdin_lock = stdin.lock();
        let mut buf = [0u8; 4096];

        while !self.is_stopped() {
            match stdin_lock.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if writer.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    writer.flush().ok();
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        Ok(())
    }

    /// Forward container output to stdout.
    #[cfg(target_os = "linux")]
    pub fn forward_stdout<R: Read>(&self, mut reader: R) -> BockResult<()> {
        if !self.config.attach_stdout {
            return Ok(());
        }

        let stdout = std::io::stdout();
        let mut stdout_lock = stdout.lock();
        let mut buf = [0u8; 4096];

        while !self.is_stopped() {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if stdout_lock.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    stdout_lock.flush().ok();
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        Ok(())
    }

    /// Setup detached mode - redirect to /dev/null.
    #[cfg(target_os = "linux")]
    pub fn setup_detached(&self) -> BockResult<()> {
        use std::fs::OpenOptions;

        if self.config.mode != StdioMode::Detached {
            return Ok(());
        }

        let dev_null = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/null")?;

        let null_fd = dev_null.as_raw_fd();

        // Redirect stdin, stdout, stderr to /dev/null
        unsafe {
            libc::dup2(null_fd, libc::STDIN_FILENO);
            libc::dup2(null_fd, libc::STDOUT_FILENO);
            libc::dup2(null_fd, libc::STDERR_FILENO);
        }

        tracing::debug!("Detached mode: stdio redirected to /dev/null");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn setup_detached(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "detached stdio".to_string(),
        })
    }
}

/// Pipe pair for container I/O.
pub struct IoPipe {
    /// Read end.
    pub read_fd: RawFd,
    /// Write end.
    pub write_fd: RawFd,
}

impl IoPipe {
    /// Create a new pipe pair.
    #[cfg(target_os = "linux")]
    pub fn new() -> BockResult<Self> {
        let mut fds = [0i32; 2];

        let result = unsafe { libc::pipe(fds.as_mut_ptr()) };

        if result < 0 {
            return Err(bock_common::BockError::Internal {
                message: format!("Failed to create pipe: {}", std::io::Error::last_os_error()),
            });
        }

        Ok(Self {
            read_fd: fds[0],
            write_fd: fds[1],
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new() -> BockResult<Self> {
        Err(bock_common::BockError::Unsupported {
            feature: "pipes".to_string(),
        })
    }

    /// Close the read end.
    pub fn close_read(&mut self) {
        if self.read_fd >= 0 {
            unsafe { libc::close(self.read_fd) };
            self.read_fd = -1;
        }
    }

    /// Close the write end.
    pub fn close_write(&mut self) {
        if self.write_fd >= 0 {
            unsafe { libc::close(self.write_fd) };
            self.write_fd = -1;
        }
    }
}

impl Drop for IoPipe {
    fn drop(&mut self) {
        self.close_read();
        self.close_write();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_config_default() {
        let config = StdioConfig::default();
        assert_eq!(config.mode, StdioMode::Attached);
        assert!(config.attach_stdin);
        assert!(config.attach_stdout);
        assert!(config.attach_stderr);
    }

    #[test]
    fn test_stdio_config_detached() {
        let config = StdioConfig::detached();
        assert_eq!(config.mode, StdioMode::Detached);
        assert!(!config.attach_stdin);
        assert!(!config.attach_stdout);
        assert!(!config.attach_stderr);
    }

    #[test]
    fn test_stdio_handler_stop() {
        let handler = StdioHandler::new(StdioConfig::default());
        assert!(!handler.is_stopped());
        handler.stop();
        assert!(handler.is_stopped());
    }
}
