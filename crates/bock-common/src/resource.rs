//! Resource quantity parsing and representation.
//!
//! Supports Kubernetes-style resource quantities:
//! - CPU: "500m" (millicores), "2" (cores), "0.5" (half core)
//! - Memory: "128Mi", "1Gi", "512M", "1G", "1024" (bytes)

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{BockError, BockResult};

/// A resource quantity with a value and unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceQuantity {
    /// The raw value in the smallest unit (millicores for CPU, bytes for memory).
    value: u64,
    /// The type of resource.
    kind: ResourceKind,
}

/// The type of resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    /// CPU in millicores.
    Cpu,
    /// Memory in bytes.
    Memory,
}

impl ResourceQuantity {
    /// Create a CPU quantity from millicores.
    #[must_use]
    pub const fn cpu_millicores(millicores: u64) -> Self {
        Self {
            value: millicores,
            kind: ResourceKind::Cpu,
        }
    }

    /// Create a CPU quantity from cores.
    #[must_use]
    pub const fn cpu_cores(cores: u64) -> Self {
        Self {
            value: cores * 1000,
            kind: ResourceKind::Cpu,
        }
    }

    /// Create a memory quantity from bytes.
    #[must_use]
    pub const fn memory_bytes(bytes: u64) -> Self {
        Self {
            value: bytes,
            kind: ResourceKind::Memory,
        }
    }

    /// Create a memory quantity from mebibytes (MiB).
    #[must_use]
    pub const fn memory_mebibytes(mib: u64) -> Self {
        Self {
            value: mib * 1024 * 1024,
            kind: ResourceKind::Memory,
        }
    }

    /// Create a memory quantity from gibibytes (GiB).
    #[must_use]
    pub const fn memory_gibibytes(gib: u64) -> Self {
        Self {
            value: gib * 1024 * 1024 * 1024,
            kind: ResourceKind::Memory,
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.value
    }

    /// Get the resource kind.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Convert CPU to millicores.
    #[must_use]
    pub const fn as_millicores(&self) -> u64 {
        self.value
    }

    /// Convert CPU to cores (truncated).
    #[must_use]
    pub const fn as_cores(&self) -> u64 {
        self.value / 1000
    }

    /// Convert memory to bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> u64 {
        self.value
    }

    /// Convert memory to mebibytes (truncated).
    #[must_use]
    pub const fn as_mebibytes(&self) -> u64 {
        self.value / (1024 * 1024)
    }

    /// Convert memory to gibibytes (truncated).
    #[must_use]
    pub const fn as_gibibytes(&self) -> u64 {
        self.value / (1024 * 1024 * 1024)
    }

    /// Parse a CPU quantity string.
    ///
    /// Formats:
    /// - "500m" -> 500 millicores
    /// - "2" -> 2000 millicores
    /// - "0.5" -> 500 millicores
    pub fn parse_cpu(s: &str) -> BockResult<Self> {
        let s = s.trim();

        if let Some(stripped) = s.strip_suffix('m') {
            let millicores: u64 =
                stripped
                    .parse()
                    .map_err(|_| BockError::InvalidResourceQuantity {
                        value: s.to_string(),
                    })?;
            return Ok(Self::cpu_millicores(millicores));
        }

        // Try parsing as float (cores)
        let cores: f64 = s.parse().map_err(|_| BockError::InvalidResourceQuantity {
            value: s.to_string(),
        })?;

        if cores < 0.0 {
            return Err(BockError::InvalidResourceQuantity {
                value: s.to_string(),
            });
        }

        Ok(Self::cpu_millicores((cores * 1000.0) as u64))
    }

    /// Parse a memory quantity string.
    ///
    /// Formats (binary - powers of 1024):
    /// - "128Ki" -> 128 * 1024 bytes
    /// - "128Mi" -> 128 * 1024^2 bytes
    /// - "1Gi" -> 1 * 1024^3 bytes
    ///
    /// Formats (decimal - powers of 1000):
    /// - "128k" -> 128 * 1000 bytes
    /// - "128M" -> 128 * 1000^2 bytes
    /// - "1G" -> 1 * 1000^3 bytes
    ///
    /// Plain number is bytes.
    pub fn parse_memory(s: &str) -> BockResult<Self> {
        let s = s.trim();

        // Binary suffixes (powers of 1024)
        let binary_suffixes = [
            ("Ki", 1024u64),
            ("Mi", 1024 * 1024),
            ("Gi", 1024 * 1024 * 1024),
            ("Ti", 1024 * 1024 * 1024 * 1024),
        ];

        for (suffix, multiplier) in binary_suffixes {
            if let Some(stripped) = s.strip_suffix(suffix) {
                let value: u64 =
                    stripped
                        .parse()
                        .map_err(|_| BockError::InvalidResourceQuantity {
                            value: s.to_string(),
                        })?;
                return Ok(Self::memory_bytes(value * multiplier));
            }
        }

        // Decimal suffixes (powers of 1000)
        let decimal_suffixes = [
            ("k", 1000u64),
            ("m", 1000 * 1000), // Note: lowercase 'm' for mega, not milli
            ("M", 1000 * 1000),
            ("g", 1000 * 1000 * 1000),
            ("G", 1000 * 1000 * 1000),
            ("t", 1000 * 1000 * 1000 * 1000),
            ("T", 1000 * 1000 * 1000 * 1000),
        ];

        for (suffix, multiplier) in decimal_suffixes {
            if let Some(stripped) = s.strip_suffix(suffix) {
                let value: u64 =
                    stripped
                        .parse()
                        .map_err(|_| BockError::InvalidResourceQuantity {
                            value: s.to_string(),
                        })?;
                return Ok(Self::memory_bytes(value * multiplier));
            }
        }

        // Plain bytes
        let bytes: u64 = s.parse().map_err(|_| BockError::InvalidResourceQuantity {
            value: s.to_string(),
        })?;
        Ok(Self::memory_bytes(bytes))
    }
}

impl fmt::Display for ResourceQuantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ResourceKind::Cpu => {
                if self.value % 1000 == 0 {
                    write!(f, "{}", self.value / 1000)
                } else {
                    write!(f, "{}m", self.value)
                }
            }
            ResourceKind::Memory => {
                const GI: u64 = 1024 * 1024 * 1024;
                const MI: u64 = 1024 * 1024;
                const KI: u64 = 1024;

                if self.value >= GI && self.value % GI == 0 {
                    write!(f, "{}Gi", self.value / GI)
                } else if self.value >= MI && self.value % MI == 0 {
                    write!(f, "{}Mi", self.value / MI)
                } else if self.value >= KI && self.value % KI == 0 {
                    write!(f, "{}Ki", self.value / KI)
                } else {
                    write!(f, "{}", self.value)
                }
            }
        }
    }
}

impl FromStr for ResourceQuantity {
    type Err = BockError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try CPU first (if it ends with 'm' or is a simple number/decimal)
        if s.ends_with('m') || s.parse::<f64>().is_ok() && !s.contains(|c: char| c.is_alphabetic())
        {
            Self::parse_cpu(s)
        } else {
            Self::parse_memory(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cpu_millicores() {
        assert_eq!(ResourceQuantity::parse_cpu("500m").unwrap().value, 500);
        assert_eq!(ResourceQuantity::parse_cpu("1000m").unwrap().value, 1000);
        assert_eq!(ResourceQuantity::parse_cpu("100m").unwrap().value, 100);
    }

    #[test]
    fn parse_cpu_cores() {
        assert_eq!(ResourceQuantity::parse_cpu("1").unwrap().value, 1000);
        assert_eq!(ResourceQuantity::parse_cpu("2").unwrap().value, 2000);
        assert_eq!(ResourceQuantity::parse_cpu("0.5").unwrap().value, 500);
        assert_eq!(ResourceQuantity::parse_cpu("1.5").unwrap().value, 1500);
    }

    #[test]
    fn parse_memory_binary() {
        assert_eq!(ResourceQuantity::parse_memory("1Ki").unwrap().value, 1024);
        assert_eq!(
            ResourceQuantity::parse_memory("128Mi").unwrap().value,
            128 * 1024 * 1024
        );
        assert_eq!(
            ResourceQuantity::parse_memory("1Gi").unwrap().value,
            1024 * 1024 * 1024
        );
    }

    #[test]
    fn parse_memory_decimal() {
        assert_eq!(ResourceQuantity::parse_memory("1k").unwrap().value, 1000);
        assert_eq!(
            ResourceQuantity::parse_memory("128M").unwrap().value,
            128 * 1000 * 1000
        );
        assert_eq!(
            ResourceQuantity::parse_memory("1G").unwrap().value,
            1000 * 1000 * 1000
        );
    }

    #[test]
    fn parse_memory_bytes() {
        assert_eq!(ResourceQuantity::parse_memory("1024").unwrap().value, 1024);
        assert_eq!(
            ResourceQuantity::parse_memory("1048576").unwrap().value,
            1048576
        );
    }

    #[test]
    fn display_cpu() {
        assert_eq!(ResourceQuantity::cpu_cores(2).to_string(), "2");
        assert_eq!(ResourceQuantity::cpu_millicores(500).to_string(), "500m");
        assert_eq!(ResourceQuantity::cpu_millicores(1500).to_string(), "1500m");
    }

    #[test]
    fn display_memory() {
        assert_eq!(ResourceQuantity::memory_gibibytes(1).to_string(), "1Gi");
        assert_eq!(ResourceQuantity::memory_mebibytes(512).to_string(), "512Mi");
        assert_eq!(ResourceQuantity::memory_bytes(1024).to_string(), "1Ki");
        assert_eq!(ResourceQuantity::memory_bytes(500).to_string(), "500");
    }
}
