//! Cgroup manager implementation.

use std::path::PathBuf;

use bock_common::BockResult;

use super::CgroupResources;

/// Default cgroup root path.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Manages a cgroup for a container.
#[derive(Debug)]
pub struct CgroupManager {
    /// Container ID.
    container_id: String,
    /// Cgroup path.
    path: PathBuf,
    /// Whether we created this cgroup.
    created: bool,
}

impl CgroupManager {
    /// Create a new cgroup for a container.
    pub fn new(container_id: &str) -> BockResult<Self> {
        let path = PathBuf::from(CGROUP_ROOT).join("bock").join(container_id);

        tracing::debug!(
            container_id = %container_id,
            path = %path.display(),
            "Creating cgroup"
        );

        // Create the cgroup directory
        std::fs::create_dir_all(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                bock_common::BockError::PermissionDenied {
                    operation: "create cgroup".to_string(),
                }
            } else {
                bock_common::BockError::Io(e)
            }
        })?;

        Ok(Self {
            container_id: container_id.to_string(),
            path,
            created: true,
        })
    }

    /// Get an existing cgroup for a container.
    pub fn get(container_id: &str) -> BockResult<Self> {
        let path = PathBuf::from(CGROUP_ROOT).join("bock").join(container_id);

        if !path.exists() {
            return Err(bock_common::BockError::ContainerNotFound {
                id: container_id.to_string(),
            });
        }

        Ok(Self {
            container_id: container_id.to_string(),
            path,
            created: false,
        })
    }

    /// Get the cgroup path.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Add a process to the cgroup.
    pub fn add_process(&self, pid: u32) -> BockResult<()> {
        let procs_path = self.path.join("cgroup.procs");
        std::fs::write(&procs_path, pid.to_string())?;

        tracing::debug!(
            container_id = %self.container_id,
            pid = pid,
            "Added process to cgroup"
        );

        Ok(())
    }

    /// Apply resource limits.
    pub fn apply_resources(&self, resources: &CgroupResources) -> BockResult<()> {
        if let Some(cpu) = &resources.cpu {
            self.apply_cpu(cpu)?;
        }

        if let Some(memory) = &resources.memory {
            self.apply_memory(memory)?;
        }

        if let Some(pids) = &resources.pids {
            self.apply_pids(pids)?;
        }

        if let Some(io) = &resources.io {
            self.apply_io(io)?;
        }

        Ok(())
    }

    /// Apply CPU limits.
    fn apply_cpu(&self, cpu: &super::CpuResources) -> BockResult<()> {
        // cpu.max format: "$quota $period"
        if let Some(quota) = cpu.quota {
            let period = cpu.period.unwrap_or(100_000);
            let value = format!("{} {}", quota, period);
            std::fs::write(self.path.join("cpu.max"), value)?;
            tracing::debug!(quota, period, "Set CPU quota");
        }

        // cpu.weight format: "$weight"
        if let Some(weight) = cpu.weight {
            std::fs::write(self.path.join("cpu.weight"), weight.to_string())?;
            tracing::debug!(weight, "Set CPU weight");
        }

        // cpuset.cpus
        if let Some(cpus) = &cpu.cpus {
            std::fs::write(self.path.join("cpuset.cpus"), cpus)?;
            tracing::debug!(cpus, "Set cpuset.cpus");
        }

        Ok(())
    }

    /// Apply memory limits.
    fn apply_memory(&self, memory: &super::MemoryResources) -> BockResult<()> {
        if let Some(max) = memory.max {
            std::fs::write(self.path.join("memory.max"), max.to_string())?;
            tracing::debug!(max, "Set memory.max");
        }

        if let Some(high) = memory.high {
            std::fs::write(self.path.join("memory.high"), high.to_string())?;
            tracing::debug!(high, "Set memory.high");
        }

        if let Some(low) = memory.low {
            std::fs::write(self.path.join("memory.low"), low.to_string())?;
            tracing::debug!(low, "Set memory.low");
        }

        if let Some(swap_max) = memory.swap_max {
            std::fs::write(self.path.join("memory.swap.max"), swap_max.to_string())?;
            tracing::debug!(swap_max, "Set memory.swap.max");
        }

        Ok(())
    }

    /// Apply PIDs limit.
    fn apply_pids(&self, pids: &super::PidsResources) -> BockResult<()> {
        std::fs::write(self.path.join("pids.max"), pids.max.to_string())?;
        tracing::debug!(max = pids.max, "Set pids.max");
        Ok(())
    }

    /// Apply I/O limits.
    fn apply_io(&self, io: &super::IoResources) -> BockResult<()> {
        if let Some(weight) = io.weight {
            std::fs::write(self.path.join("io.weight"), format!("default {}", weight))?;
            tracing::debug!(weight, "Set io.weight");
        }

        // Apply per-device read/write limits using io.max
        // Format: "MAJOR:MINOR rbps=LIMIT wbps=LIMIT riops=max wiops=max"
        for (device, limit) in &io.read_bps {
            let value = format!("{} rbps={}", device, limit);
            if let Err(e) = std::fs::write(self.path.join("io.max"), &value) {
                tracing::warn!(device, limit, error = %e, "Failed to set read BPS limit");
            } else {
                tracing::debug!(device, limit, "Set io.max read BPS");
            }
        }

        for (device, limit) in &io.write_bps {
            let value = format!("{} wbps={}", device, limit);
            if let Err(e) = std::fs::write(self.path.join("io.max"), &value) {
                tracing::warn!(device, limit, error = %e, "Failed to set write BPS limit");
            } else {
                tracing::debug!(device, limit, "Set io.max write BPS");
            }
        }

        // Apply IOPS limits if provided
        for (device, limit) in &io.read_iops {
            let value = format!("{} riops={}", device, limit);
            if let Err(e) = std::fs::write(self.path.join("io.max"), &value) {
                tracing::warn!(device, limit, error = %e, "Failed to set read IOPS limit");
            } else {
                tracing::debug!(device, limit, "Set io.max read IOPS");
            }
        }

        for (device, limit) in &io.write_iops {
            let value = format!("{} wiops={}", device, limit);
            if let Err(e) = std::fs::write(self.path.join("io.max"), &value) {
                tracing::warn!(device, limit, error = %e, "Failed to set write IOPS limit");
            } else {
                tracing::debug!(device, limit, "Set io.max write IOPS");
            }
        }

        Ok(())
    }

    /// Freeze the cgroup (pause containers).
    pub fn freeze(&self) -> BockResult<()> {
        std::fs::write(self.path.join("cgroup.freeze"), "1")?;
        tracing::debug!(container_id = %self.container_id, "Froze cgroup");
        Ok(())
    }

    /// Unfreeze the cgroup (resume containers).
    pub fn unfreeze(&self) -> BockResult<()> {
        std::fs::write(self.path.join("cgroup.freeze"), "0")?;
        tracing::debug!(container_id = %self.container_id, "Unfroze cgroup");
        Ok(())
    }

    /// Get current memory usage.
    pub fn memory_usage(&self) -> BockResult<u64> {
        let content = std::fs::read_to_string(self.path.join("memory.current"))?;
        let bytes: u64 = content.trim().parse().unwrap_or(0);
        Ok(bytes)
    }

    /// Get current CPU statistics.
    pub fn cpu_stats(&self) -> BockResult<CpuStats> {
        let content = std::fs::read_to_string(self.path.join("cpu.stat"))?;
        let mut stats = CpuStats::default();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                match parts[0] {
                    "usage_usec" => stats.usage_usec = parts[1].parse().unwrap_or(0),
                    "user_usec" => stats.user_usec = parts[1].parse().unwrap_or(0),
                    "system_usec" => stats.system_usec = parts[1].parse().unwrap_or(0),
                    _ => {}
                }
            }
        }

        Ok(stats)
    }

    /// Kill all processes in the cgroup.
    pub fn kill_all(&self) -> BockResult<()> {
        std::fs::write(self.path.join("cgroup.kill"), "1")?;
        tracing::debug!(container_id = %self.container_id, "Killed all processes in cgroup");
        Ok(())
    }

    /// Delete the cgroup.
    pub fn delete(&self) -> BockResult<()> {
        if self.created && self.path.exists() {
            std::fs::remove_dir(&self.path)?;
            tracing::debug!(
                container_id = %self.container_id,
                path = %self.path.display(),
                "Deleted cgroup"
            );
        }
        Ok(())
    }
}

/// CPU statistics.
#[derive(Debug, Default)]
pub struct CpuStats {
    /// Total CPU usage in microseconds.
    pub usage_usec: u64,
    /// User CPU time in microseconds.
    pub user_usec: u64,
    /// System CPU time in microseconds.
    pub system_usec: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require root privileges and cgroup v2
    #[test]
    #[ignore = "requires root and cgroups v2"]
    fn create_and_delete_cgroup() {
        let manager = CgroupManager::new("test-container").unwrap();
        assert!(manager.path().exists());
        manager.delete().unwrap();
    }
}
