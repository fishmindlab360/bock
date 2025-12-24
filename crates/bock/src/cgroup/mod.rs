//! Cgroup v2 management.
//!
//! This module provides utilities for managing Linux cgroups v2.

mod manager;
pub mod v1;

pub use manager::CgroupManager;
pub use v1::{CgroupV1Manager, CgroupVersion, MemoryPressure, MemoryPressureMonitor, MemoryUsage};

/// Cgroup resource configuration.
#[derive(Debug, Clone, Default)]
pub struct CgroupResources {
    /// CPU resources.
    pub cpu: Option<CpuResources>,
    /// Memory resources.
    pub memory: Option<MemoryResources>,
    /// PIDs limit.
    pub pids: Option<PidsResources>,
    /// Block I/O resources.
    pub io: Option<IoResources>,
}

/// CPU resource limits.
#[derive(Debug, Clone)]
pub struct CpuResources {
    /// CPU quota in microseconds.
    pub quota: Option<u64>,
    /// CPU period in microseconds (default: 100000).
    pub period: Option<u64>,
    /// CPU weight (1-10000, default: 100).
    pub weight: Option<u64>,
    /// CPUs to use (e.g., "0-2").
    pub cpus: Option<String>,
}

/// Memory resource limits.
#[derive(Debug, Clone)]
pub struct MemoryResources {
    /// Hard memory limit in bytes.
    pub max: Option<u64>,
    /// High memory threshold (throttling starts).
    pub high: Option<u64>,
    /// Low memory threshold (reclaim protection).
    pub low: Option<u64>,
    /// Memory + swap limit.
    pub swap_max: Option<u64>,
}

/// PIDs resource limits.
#[derive(Debug, Clone)]
pub struct PidsResources {
    /// Maximum number of PIDs.
    pub max: u64,
}

/// Block I/O resource limits.
#[derive(Debug, Clone, Default)]
pub struct IoResources {
    /// I/O weight (1-10000, default: 100).
    pub weight: Option<u64>,
    /// Read BPS limit per device (device path, limit).
    pub read_bps: Vec<(String, u64)>,
    /// Write BPS limit per device (device path, limit).
    pub write_bps: Vec<(String, u64)>,
    /// Read IOPS limit per device (device path, limit).
    pub read_iops: Vec<(String, u64)>,
    /// Write IOPS limit per device (device path, limit).
    pub write_iops: Vec<(String, u64)>,
}
