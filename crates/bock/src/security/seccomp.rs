//! Seccomp syscall filtering.
//!
//! This module provides seccomp BPF filter generation and application
//! for restricting syscalls in containers.

#![allow(unsafe_code)]

use std::collections::HashMap;

use bock_common::BockResult;

/// Seccomp filter configuration.
#[derive(Debug, Clone)]
pub struct SeccompFilter {
    /// Default action.
    pub default_action: SeccompAction,
    /// Syscall rules.
    pub rules: Vec<SeccompRule>,
}

/// Seccomp action.
#[derive(Debug, Clone, Copy)]
pub enum SeccompAction {
    /// Allow the syscall.
    Allow,
    /// Return an error.
    Errno(u32),
    /// Send a signal.
    Trap,
    /// Kill the process.
    Kill,
    /// Log and continue.
    Log,
}

impl SeccompAction {
    /// Convert to seccompiler action.
    #[allow(unused)]
    #[cfg(target_os = "linux")]
    fn to_seccompiler(&self) -> seccompiler::SeccompAction {
        match self {
            Self::Allow => seccompiler::SeccompAction::Allow,
            Self::Errno(errno) => seccompiler::SeccompAction::Errno(*errno),
            Self::Trap => seccompiler::SeccompAction::Trap,
            Self::Kill => seccompiler::SeccompAction::KillProcess,
            Self::Log => seccompiler::SeccompAction::Log,
        }
    }
}

/// Seccomp rule for a syscall.
#[derive(Debug, Clone)]
pub struct SeccompRule {
    /// Syscall name.
    pub syscall: String,
    /// Action to take.
    pub action: SeccompAction,
}

impl SeccompFilter {
    /// Create a default-deny filter with common allowed syscalls.
    #[must_use]
    pub fn default_deny() -> Self {
        let allowed_syscalls = vec![
            "read",
            "write",
            "open",
            "close",
            "stat",
            "fstat",
            "lstat",
            "poll",
            "lseek",
            "mmap",
            "mprotect",
            "munmap",
            "brk",
            "rt_sigaction",
            "rt_sigprocmask",
            "rt_sigreturn",
            "ioctl",
            "access",
            "pipe",
            "select",
            "sched_yield",
            "mremap",
            "msync",
            "mincore",
            "madvise",
            "dup",
            "dup2",
            "pause",
            "nanosleep",
            "getitimer",
            "alarm",
            "setitimer",
            "getpid",
            "sendfile",
            "socket",
            "connect",
            "accept",
            "sendto",
            "recvfrom",
            "sendmsg",
            "recvmsg",
            "shutdown",
            "bind",
            "listen",
            "getsockname",
            "getpeername",
            "socketpair",
            "setsockopt",
            "getsockopt",
            "clone",
            "fork",
            "vfork",
            "execve",
            "exit",
            "wait4",
            "kill",
            "uname",
            "fcntl",
            "flock",
            "fsync",
            "fdatasync",
            "truncate",
            "ftruncate",
            "getdents",
            "getcwd",
            "chdir",
            "fchdir",
            "rename",
            "mkdir",
            "rmdir",
            "creat",
            "link",
            "unlink",
            "symlink",
            "readlink",
            "chmod",
            "fchmod",
            "chown",
            "fchown",
            "lchown",
            "umask",
            "gettimeofday",
            "getrlimit",
            "getrusage",
            "sysinfo",
            "times",
            "getuid",
            "getgid",
            "setuid",
            "setgid",
            "geteuid",
            "getegid",
            "setpgid",
            "getppid",
            "getpgrp",
            "setsid",
            "setreuid",
            "setregid",
            "getgroups",
            "setgroups",
            "setresuid",
            "getresuid",
            "setresgid",
            "getresgid",
            "getpgid",
            "setfsuid",
            "setfsgid",
            "getsid",
            "capget",
            "capset",
            "rt_sigpending",
            "rt_sigtimedwait",
            "rt_sigqueueinfo",
            "rt_sigsuspend",
            "sigaltstack",
            "utime",
            "mknod",
            "statfs",
            "fstatfs",
            "getpriority",
            "setpriority",
            "sched_setparam",
            "sched_getparam",
            "sched_setscheduler",
            "sched_getscheduler",
            "sched_get_priority_max",
            "sched_get_priority_min",
            "sched_rr_get_interval",
            "mlock",
            "munlock",
            "mlockall",
            "munlockall",
            "vhangup",
            "pivot_root",
            "prctl",
            "arch_prctl",
            "setrlimit",
            "sync",
            "mount",
            "umount2",
            "sethostname",
            "setdomainname",
            "gettid",
            "readahead",
            "setxattr",
            "lsetxattr",
            "fsetxattr",
            "getxattr",
            "lgetxattr",
            "fgetxattr",
            "listxattr",
            "llistxattr",
            "flistxattr",
            "removexattr",
            "lremovexattr",
            "fremovexattr",
            "tkill",
            "time",
            "futex",
            "sched_setaffinity",
            "sched_getaffinity",
            "set_thread_area",
            "get_thread_area",
            "io_setup",
            "io_destroy",
            "io_getevents",
            "io_submit",
            "io_cancel",
            "exit_group",
            "epoll_create",
            "epoll_ctl",
            "epoll_wait",
            "set_tid_address",
            "fadvise64",
            "timer_create",
            "timer_settime",
            "timer_gettime",
            "timer_getoverrun",
            "timer_delete",
            "clock_settime",
            "clock_gettime",
            "clock_getres",
            "clock_nanosleep",
            "tgkill",
            "utimes",
            "openat",
            "mkdirat",
            "mknodat",
            "fchownat",
            "futimesat",
            "newfstatat",
            "unlinkat",
            "renameat",
            "linkat",
            "symlinkat",
            "readlinkat",
            "fchmodat",
            "faccessat",
            "pselect6",
            "ppoll",
            "set_robust_list",
            "get_robust_list",
            "splice",
            "tee",
            "sync_file_range",
            "vmsplice",
            "utimensat",
            "epoll_pwait",
            "signalfd",
            "timerfd_create",
            "eventfd",
            "fallocate",
            "timerfd_settime",
            "timerfd_gettime",
            "accept4",
            "signalfd4",
            "eventfd2",
            "epoll_create1",
            "dup3",
            "pipe2",
            "inotify_init1",
            "preadv",
            "pwritev",
            "rt_tgsigqueueinfo",
            "recvmmsg",
            "prlimit64",
            "syncfs",
            "sendmmsg",
            "setns",
            "getcpu",
            "getrandom",
            "memfd_create",
            "execveat",
            "mlock2",
            "copy_file_range",
            "preadv2",
            "pwritev2",
            "statx",
            "rseq",
            "pidfd_open",
            "clone3",
            "close_range",
            "pidfd_getfd",
            "faccessat2",
            "epoll_pwait2",
            "openat2",
            "futex_waitv",
            "getdents64",
            "seccomp",
            "pread64",
            "pwrite64",
        ];

        let rules = allowed_syscalls
            .into_iter()
            .map(|syscall| SeccompRule {
                syscall: syscall.to_string(),
                action: SeccompAction::Allow,
            })
            .collect();

        Self {
            default_action: SeccompAction::Errno(1), // EPERM
            rules,
        }
    }

    /// Create a permissive filter that logs denied syscalls.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            default_action: SeccompAction::Log,
            rules: Vec::new(),
        }
    }

    /// Apply the seccomp filter to the current process.
    #[cfg(target_os = "linux")]
    pub fn apply(&self) -> BockResult<()> {
        use seccompiler::{BpfProgram, compile_from_json};

        tracing::debug!(
            default_action = ?self.default_action,
            num_rules = self.rules.len(),
            "Applying seccomp filter"
        );

        // Determine target architecture
        let target_arch = Self::get_target_arch()?;

        // Build the filter JSON structure that seccompiler expects
        let filter_json = self.to_json_filter();

        // Compile to BPF
        let bpf_map: HashMap<String, BpfProgram> =
            compile_from_json(filter_json.as_bytes(), target_arch).map_err(|e| {
                bock_common::BockError::Internal {
                    message: format!("Failed to compile seccomp filter: {}", e),
                }
            })?;

        // Get the default filter (we use "default" as the filter name)
        let bpf_prog = bpf_map
            .get("default")
            .ok_or_else(|| bock_common::BockError::Internal {
                message: "Seccomp filter compilation produced no output".to_string(),
            })?;

        // Apply the filter
        seccompiler::apply_filter(bpf_prog).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to apply seccomp filter: {}", e),
        })?;

        tracing::info!("Seccomp filter applied successfully");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn apply(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "seccomp".to_string(),
        })
    }

    /// Get the target architecture for seccomp.
    #[cfg(target_os = "linux")]
    fn get_target_arch() -> BockResult<seccompiler::TargetArch> {
        #[cfg(target_arch = "x86_64")]
        return Ok(seccompiler::TargetArch::x86_64);

        #[cfg(target_arch = "aarch64")]
        return Ok(seccompiler::TargetArch::aarch64);

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        return Err(bock_common::BockError::Unsupported {
            feature: format!("seccomp on {}", std::env::consts::ARCH),
        });
    }

    /// Convert filter to JSON format expected by seccompiler.
    #[cfg(target_os = "linux")]
    fn to_json_filter(&self) -> String {
        use serde_json::json;

        // Build rules array
        let mut syscall_rules: HashMap<String, serde_json::Value> = HashMap::new();

        for rule in &self.rules {
            let action = match rule.action {
                SeccompAction::Allow => "allow",
                SeccompAction::Errno(_) => "errno",
                SeccompAction::Trap => "trap",
                SeccompAction::Kill => "kill_process",
                SeccompAction::Log => "log",
            };

            syscall_rules.insert(
                rule.syscall.clone(),
                json!({
                    "action": action,
                }),
            );
        }

        let default_action = match self.default_action {
            SeccompAction::Allow => "allow",
            SeccompAction::Errno(_) => "errno",
            SeccompAction::Trap => "trap",
            SeccompAction::Kill => "kill_process",
            SeccompAction::Log => "log",
        };

        // Build the filter map in seccompiler's expected format
        let filter = json!({
            "default": {
                "default_action": default_action,
                "filter_action": "allow",
                "filter": []
            }
        });

        serde_json::to_string(&filter).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_deny_filter() {
        let filter = SeccompFilter::default_deny();
        assert!(!filter.rules.is_empty());
        assert!(matches!(filter.default_action, SeccompAction::Errno(1)));
    }

    #[test]
    fn test_permissive_filter() {
        let filter = SeccompFilter::permissive();
        assert!(filter.rules.is_empty());
        assert!(matches!(filter.default_action, SeccompAction::Log));
    }

    #[test]
    fn test_action_conversion() {
        let action = SeccompAction::Allow;
        assert!(matches!(action, SeccompAction::Allow));

        let errno_action = SeccompAction::Errno(13); // EACCES
        if let SeccompAction::Errno(n) = errno_action {
            assert_eq!(n, 13);
        }
    }
}
