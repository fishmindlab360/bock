//! Cgroup v1 fallback support.
//!
//! This module provides fallback support for cgroups v1 systems
//! when cgroups v2 is not available.

use std::fs;
use std::path::{Path, PathBuf};

use bock_common::BockResult;

/// Cgroup version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgroupVersion {
    /// Cgroups v1 (legacy).
    V1,
    /// Cgroups v2 (unified).
    V2,
    /// Hybrid mode (v1 + v2).
    Hybrid,
}

impl CgroupVersion {
    /// Detect the cgroup version on the system.
    pub fn detect() -> Self {
        let cgroup2_path = Path::new("/sys/fs/cgroup/cgroup.controllers");
        let cgroup1_path = Path::new("/sys/fs/cgroup/cpu");

        if cgroup2_path.exists() && !cgroup1_path.exists() {
            Self::V2
        } else if cgroup2_path.exists() && cgroup1_path.exists() {
            Self::Hybrid
        } else {
            Self::V1
        }
    }
}

/// Cgroup v1 manager.
pub struct CgroupV1Manager {
    /// Container ID.
    container_id: String,
    /// Controllers to manage.
    controllers: Vec<String>,
}

impl CgroupV1Manager {
    /// Create a new v1 cgroup manager.
    pub fn new(container_id: &str) -> BockResult<Self> {
        let controllers = Self::available_controllers()?;

        Ok(Self {
            container_id: container_id.to_string(),
            controllers,
        })
    }

    /// Get available cgroup controllers.
    fn available_controllers() -> BockResult<Vec<String>> {
        let mut controllers = Vec::new();

        let cgroup_base = Path::new("/sys/fs/cgroup");

        for entry in fs::read_dir(cgroup_base)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    // Skip unified cgroup2 mount
                    if name_str != "unified" && name_str != "systemd" {
                        controllers.push(name_str.to_string());
                    }
                }
            }
        }

        Ok(controllers)
    }

    /// Get the path for a specific controller.
    fn controller_path(&self, controller: &str) -> PathBuf {
        Path::new("/sys/fs/cgroup")
            .join(controller)
            .join("bock")
            .join(&self.container_id)
    }

    /// Create cgroup directories for all controllers.
    pub fn create(&self) -> BockResult<()> {
        for controller in &self.controllers {
            let path = self.controller_path(controller);
            fs::create_dir_all(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    bock_common::BockError::PermissionDenied {
                        operation: format!("create cgroup {}", controller),
                    }
                } else {
                    bock_common::BockError::Io(e)
                }
            })?;
        }

        tracing::debug!(
            container_id = %self.container_id,
            controllers = ?self.controllers,
            "Created v1 cgroups"
        );
        Ok(())
    }

    /// Add a process to all cgroups.
    pub fn add_process(&self, pid: u32) -> BockResult<()> {
        for controller in &self.controllers {
            let tasks_path = self.controller_path(controller).join("tasks");
            fs::write(&tasks_path, pid.to_string())?;
        }

        tracing::debug!(pid, "Process added to v1 cgroups");
        Ok(())
    }

    /// Apply CPU limits.
    pub fn apply_cpu(&self, quota: Option<u64>, period: Option<u64>) -> BockResult<()> {
        if !self.controllers.contains(&"cpu".to_string()) {
            return Ok(());
        }

        let cpu_path = self.controller_path("cpu");

        if let Some(period) = period {
            fs::write(cpu_path.join("cpu.cfs_period_us"), period.to_string())?;
        }

        if let Some(quota) = quota {
            fs::write(cpu_path.join("cpu.cfs_quota_us"), quota.to_string())?;
        }

        Ok(())
    }

    /// Apply memory limits.
    pub fn apply_memory(&self, limit: Option<u64>) -> BockResult<()> {
        if !self.controllers.contains(&"memory".to_string()) {
            return Ok(());
        }

        let memory_path = self.controller_path("memory");

        if let Some(limit) = limit {
            fs::write(memory_path.join("memory.limit_in_bytes"), limit.to_string())?;
        }

        Ok(())
    }

    /// Apply PIDs limit.
    pub fn apply_pids(&self, max: u64) -> BockResult<()> {
        if !self.controllers.contains(&"pids".to_string()) {
            return Ok(());
        }

        let pids_path = self.controller_path("pids");
        fs::write(pids_path.join("pids.max"), max.to_string())?;

        Ok(())
    }

    /// Delete the cgroups.
    pub fn delete(&self) -> BockResult<()> {
        for controller in &self.controllers {
            let path = self.controller_path(controller);
            if path.exists() {
                fs::remove_dir(&path).ok();
            }
        }

        Ok(())
    }
}

/// Memory pressure monitor.
pub struct MemoryPressureMonitor {
    /// Cgroup path.
    cgroup_path: PathBuf,
}

impl MemoryPressureMonitor {
    /// Create a new memory pressure monitor.
    pub fn new(cgroup_path: &Path) -> Self {
        Self {
            cgroup_path: cgroup_path.to_path_buf(),
        }
    }

    /// Get current memory pressure level.
    #[cfg(target_os = "linux")]
    pub fn get_pressure(&self) -> BockResult<MemoryPressure> {
        let pressure_path = self.cgroup_path.join("memory.pressure");

        if !pressure_path.exists() {
            return Ok(MemoryPressure::default());
        }

        let content = fs::read_to_string(&pressure_path)?;
        MemoryPressure::parse(&content)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_pressure(&self) -> BockResult<MemoryPressure> {
        Ok(MemoryPressure::default())
    }

    /// Get memory usage stats.
    pub fn get_usage(&self) -> BockResult<MemoryUsage> {
        let current_path = self.cgroup_path.join("memory.current");
        let max_path = self.cgroup_path.join("memory.max");

        let current = fs::read_to_string(&current_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        let max = fs::read_to_string(&max_path).ok().and_then(|s| {
            let s = s.trim();
            if s == "max" { None } else { s.parse().ok() }
        });

        Ok(MemoryUsage { current, max })
    }
}

/// Memory pressure statistics.
#[derive(Debug, Clone, Default)]
pub struct MemoryPressure {
    /// Average pressure over 10 seconds.
    pub avg10: f64,
    /// Average pressure over 60 seconds.
    pub avg60: f64,
    /// Average pressure over 300 seconds.
    pub avg300: f64,
    /// Total stall time in microseconds.
    pub total: u64,
}

impl MemoryPressure {
    /// Parse pressure from cgroup file content.
    pub fn parse(content: &str) -> BockResult<Self> {
        let mut pressure = Self::default();

        for line in content.lines() {
            if line.starts_with("some") || line.starts_with("full") {
                for part in line.split_whitespace().skip(1) {
                    if let Some((key, value)) = part.split_once('=') {
                        match key {
                            "avg10" => pressure.avg10 = value.parse().unwrap_or(0.0),
                            "avg60" => pressure.avg60 = value.parse().unwrap_or(0.0),
                            "avg300" => pressure.avg300 = value.parse().unwrap_or(0.0),
                            "total" => pressure.total = value.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(pressure)
    }
}

/// Memory usage statistics.
#[derive(Debug, Clone, Default)]
pub struct MemoryUsage {
    /// Current memory usage in bytes.
    pub current: u64,
    /// Maximum memory limit (None if unlimited).
    pub max: Option<u64>,
}

impl MemoryUsage {
    /// Get usage percentage.
    pub fn percent(&self) -> Option<f64> {
        self.max
            .map(|max| (self.current as f64 / max as f64) * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgroup_version_values() {
        // Just test the enum exists
        let _ = CgroupVersion::V1;
        let _ = CgroupVersion::V2;
        let _ = CgroupVersion::Hybrid;
    }

    #[test]
    fn test_memory_pressure_parse() {
        let content = "some avg10=0.00 avg60=0.00 avg300=0.00 total=0\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0";
        let pressure = MemoryPressure::parse(content).unwrap();
        assert_eq!(pressure.avg10, 0.0);
        assert_eq!(pressure.total, 0);
    }

    #[test]
    fn test_memory_usage_percent() {
        let usage = MemoryUsage {
            current: 500,
            max: Some(1000),
        };
        assert_eq!(usage.percent(), Some(50.0));
    }
}
