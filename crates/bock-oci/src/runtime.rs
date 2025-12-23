//! OCI Runtime Specification types.
//!
//! Based on the OCI Runtime Specification v1.2.0:
//! <https://github.com/opencontainers/runtime-spec/blob/main/config.md>

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// OCI Runtime Specification (config.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    /// OCI version.
    #[serde(default = "default_oci_version")]
    pub oci_version: String,

    /// Container's root filesystem.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Root>,

    /// Container process configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,

    /// Container hostname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    /// Additional mounts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mounts: Vec<Mount>,

    /// Lifecycle hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Hooks>,

    /// Annotations (key-value pairs).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,

    /// Linux-specific configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<Linux>,
}

fn default_oci_version() -> String {
    "1.2.0".to_string()
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            oci_version: default_oci_version(),
            root: None,
            process: None,
            hostname: None,
            mounts: Vec::new(),
            hooks: None,
            annotations: HashMap::new(),
            linux: None,
        }
    }
}

/// Root filesystem configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    /// Path to the root filesystem.
    pub path: PathBuf,

    /// Whether the root filesystem is read-only.
    #[serde(default)]
    pub readonly: bool,
}

/// Process configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    /// Whether to run with a terminal.
    #[serde(default)]
    pub terminal: bool,

    /// Console size (if terminal is true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console_size: Option<ConsoleSize>,

    /// User to run as.
    pub user: User,

    /// Command arguments.
    pub args: Vec<String>,

    /// Command path (optional, derived from args[0] if not set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_line: Option<String>,

    /// Environment variables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,

    /// Working directory.
    pub cwd: PathBuf,

    /// Capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,

    /// Resource limits (rlimits).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rlimits: Vec<Rlimit>,

    /// No new privileges flag.
    #[serde(default)]
    pub no_new_privileges: bool,

    /// AppArmor profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apparmor_profile: Option<String>,

    /// OOM score adjustment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oom_score_adj: Option<i32>,

    /// SELinux label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selinux_label: Option<String>,
}

/// Console size.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConsoleSize {
    /// Height in characters.
    pub height: u32,
    /// Width in characters.
    pub width: u32,
}

/// User and group IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    /// User ID.
    pub uid: u32,
    /// Group ID.
    pub gid: u32,
    /// Umask.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub umask: Option<u32>,
    /// Additional group IDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_gids: Vec<u32>,
}

impl Default for User {
    fn default() -> Self {
        Self {
            uid: 0,
            gid: 0,
            umask: None,
            additional_gids: Vec::new(),
        }
    }
}

/// Linux capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// Bounding capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounding: Vec<String>,
    /// Effective capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effective: Vec<String>,
    /// Inheritable capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inheritable: Vec<String>,
    /// Permitted capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permitted: Vec<String>,
    /// Ambient capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ambient: Vec<String>,
}

/// Resource limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rlimit {
    /// Limit type (e.g., RLIMIT_NOFILE).
    #[serde(rename = "type")]
    pub limit_type: String,
    /// Hard limit.
    pub hard: u64,
    /// Soft limit.
    pub soft: u64,
}

/// Mount configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    /// Mount destination path (inside container).
    pub destination: PathBuf,
    /// Mount type (e.g., "bind", "tmpfs", "proc").
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub mount_type: Option<String>,
    /// Mount source path (outside container).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PathBuf>,
    /// Mount options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

/// Lifecycle hooks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hooks {
    /// Hooks run before start (in runtime namespace).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prestart: Vec<Hook>,
    /// Hooks run after container is created but before user-specified process.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub create_runtime: Vec<Hook>,
    /// Hooks run in the container namespace after pivot_root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub create_container: Vec<Hook>,
    /// Hooks run before execv of user process.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub start_container: Vec<Hook>,
    /// Hooks run after user process exits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poststart: Vec<Hook>,
    /// Hooks run after container is deleted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poststop: Vec<Hook>,
}

/// A lifecycle hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    /// Path to the hook executable.
    pub path: PathBuf,
    /// Arguments to the hook.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Environment variables for the hook.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    /// Timeout in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
}

/// Linux-specific configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Linux {
    /// UID mappings (for user namespaces).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uid_mappings: Vec<IdMapping>,
    /// GID mappings (for user namespaces).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gid_mappings: Vec<IdMapping>,
    /// Namespaces to create/join.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub namespaces: Vec<Namespace>,
    /// Devices to create.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,
    /// Cgroup path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroups_path: Option<String>,
    /// Resource limits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
    /// Seccomp configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seccomp: Option<Seccomp>,
    /// Rootfs propagation mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rootfs_propagation: Option<String>,
    /// Masked paths (hidden from container).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masked_paths: Vec<String>,
    /// Read-only paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readonly_paths: Vec<String>,
}

/// ID mapping for user/group namespaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdMapping {
    /// Container ID (start of range).
    pub container_id: u32,
    /// Host ID (start of range).
    pub host_id: u32,
    /// Size of the range.
    pub size: u32,
}

/// Namespace configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    /// Namespace type.
    #[serde(rename = "type")]
    pub ns_type: NamespaceType,
    /// Path to existing namespace (to join instead of create).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

/// Namespace types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamespaceType {
    /// PID namespace.
    Pid,
    /// Network namespace.
    Network,
    /// Mount namespace.
    Mount,
    /// IPC namespace.
    Ipc,
    /// UTS namespace.
    Uts,
    /// User namespace.
    User,
    /// Cgroup namespace.
    Cgroup,
    /// Time namespace.
    Time,
}

/// Device configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Device path.
    pub path: PathBuf,
    /// Device type (c for char, b for block).
    #[serde(rename = "type")]
    pub device_type: String,
    /// Major number.
    pub major: i64,
    /// Minor number.
    pub minor: i64,
    /// File mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_mode: Option<u32>,
    /// UID of the device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    /// GID of the device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
}

/// Resource limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Resources {
    /// CPU resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuResources>,
    /// Memory resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryResources>,
    /// PIDs limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pids: Option<PidsResources>,
    /// Block I/O resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_io: Option<BlockIoResources>,
}

/// CPU resource limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuResources {
    /// CPU shares (relative weight).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<u64>,
    /// CPU quota (in microseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota: Option<i64>,
    /// CPU period (in microseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,
    /// CPUs to use (e.g., "0-2,4").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,
    /// Memory nodes to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mems: Option<String>,
}

/// Memory resource limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryResources {
    /// Hard memory limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Memory reservation (soft limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservation: Option<i64>,
    /// Memory + swap limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap: Option<i64>,
    /// Kernel memory limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel: Option<i64>,
    /// Disable OOM killer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_oom_killer: Option<bool>,
}

/// PIDs resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidsResources {
    /// Maximum number of PIDs.
    pub limit: i64,
}

/// Block I/O resource limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockIoResources {
    /// Block I/O weight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    /// Throttle read BPS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throttle_read_bps_device: Vec<ThrottleDevice>,
    /// Throttle write BPS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throttle_write_bps_device: Vec<ThrottleDevice>,
    /// Throttle read IOPS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throttle_read_iops_device: Vec<ThrottleDevice>,
    /// Throttle write IOPS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throttle_write_iops_device: Vec<ThrottleDevice>,
}

/// Block I/O throttle configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleDevice {
    /// Major device number.
    pub major: i64,
    /// Minor device number.
    pub minor: i64,
    /// Rate limit.
    pub rate: u64,
}

/// Seccomp configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seccomp {
    /// Default action.
    pub default_action: SeccompAction,
    /// Architectures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architectures: Vec<String>,
    /// Flags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    /// Syscall rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub syscalls: Vec<SeccompSyscall>,
}

/// Seccomp action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SeccompAction {
    /// Kill the process.
    ScmpActKill,
    /// Kill the thread.
    ScmpActKillThread,
    /// Send SIGSYS.
    ScmpActTrap,
    /// Return an error.
    ScmpActErrno,
    /// Log and continue.
    ScmpActLog,
    /// Allow the syscall.
    ScmpActAllow,
    /// Notify userspace.
    ScmpActNotify,
}

/// Seccomp syscall rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompSyscall {
    /// Syscall names.
    pub names: Vec<String>,
    /// Action to take.
    pub action: SeccompAction,
    /// Errno to return (for SCMP_ACT_ERRNO).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errno_ret: Option<u32>,
    /// Argument conditions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<SeccompArg>,
}

/// Seccomp argument condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeccompArg {
    /// Argument index.
    pub index: u32,
    /// Value to compare.
    pub value: u64,
    /// Second value (for masked equality).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_two: Option<u64>,
    /// Comparison operator.
    pub op: SeccompOperator,
}

/// Seccomp comparison operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SeccompOperator {
    /// Not equal.
    ScmpCmpNe,
    /// Less than.
    ScmpCmpLt,
    /// Less than or equal.
    ScmpCmpLe,
    /// Equal.
    ScmpCmpEq,
    /// Greater than or equal.
    ScmpCmpGe,
    /// Greater than.
    ScmpCmpGt,
    /// Masked equality.
    ScmpCmpMaskedEq,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_default() {
        let spec = Spec::default();
        assert_eq!(spec.oci_version, "1.2.0");
        assert!(spec.root.is_none());
        assert!(spec.process.is_none());
    }

    #[test]
    fn spec_serialization() {
        let spec = Spec {
            root: Some(Root {
                path: "/rootfs".into(),
                readonly: true,
            }),
            hostname: Some("test-container".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&spec).unwrap();
        assert!(json.contains("rootfs"));
        assert!(json.contains("test-container"));

        let parsed: Spec = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hostname.unwrap(), "test-container");
    }

    #[test]
    fn namespace_type_serialization() {
        let ns = Namespace {
            ns_type: NamespaceType::Pid,
            path: None,
        };
        let json = serde_json::to_string(&ns).unwrap();
        assert!(json.contains("\"type\":\"pid\""));
    }
}
