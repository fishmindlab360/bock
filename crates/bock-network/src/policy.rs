//! Network policies and firewalling.
//!
//! This module provides network policy enforcement using iptables
//! for container traffic control.

use std::process::Command;

use bock_common::BockResult;

/// Network policy action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    /// Allow traffic.
    Allow,
    /// Deny traffic.
    Deny,
    /// Drop traffic silently.
    Drop,
    /// Log and allow.
    Log,
}

impl PolicyAction {
    fn as_iptables(&self) -> &'static str {
        match self {
            Self::Allow => "ACCEPT",
            Self::Deny => "REJECT",
            Self::Drop => "DROP",
            Self::Log => "LOG",
        }
    }
}

/// Network policy rule.
#[derive(Debug, Clone)]
pub struct PolicyRule {
    /// Source network/IP.
    pub source: Option<String>,
    /// Destination network/IP.
    pub destination: Option<String>,
    /// Protocol (tcp, udp, icmp).
    pub protocol: Option<String>,
    /// Destination port.
    pub port: Option<u16>,
    /// Action to take.
    pub action: PolicyAction,
    /// Rule comment.
    pub comment: Option<String>,
}

impl PolicyRule {
    /// Create an allow rule.
    pub fn allow() -> Self {
        Self {
            source: None,
            destination: None,
            protocol: None,
            port: None,
            action: PolicyAction::Allow,
            comment: None,
        }
    }

    /// Create a deny rule.
    pub fn deny() -> Self {
        Self {
            action: PolicyAction::Deny,
            ..Self::allow()
        }
    }

    /// Set source.
    pub fn from(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    /// Set destination.
    pub fn to(mut self, destination: &str) -> Self {
        self.destination = Some(destination.to_string());
        self
    }

    /// Set protocol.
    pub fn protocol(mut self, protocol: &str) -> Self {
        self.protocol = Some(protocol.to_string());
        self
    }

    /// Set port.
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Set comment.
    pub fn comment(mut self, comment: &str) -> Self {
        self.comment = Some(comment.to_string());
        self
    }
}

/// Network policy manager.
pub struct NetworkPolicy {
    /// Container ID for rule tracking.
    container_id: String,
    /// Applied rules.
    rules: Vec<PolicyRule>,
}

impl NetworkPolicy {
    /// Create a new network policy manager.
    pub fn new(container_id: &str) -> Self {
        Self {
            container_id: container_id.to_string(),
            rules: Vec::new(),
        }
    }

    /// Add a policy rule.
    #[cfg(target_os = "linux")]
    pub fn add_rule(&mut self, rule: PolicyRule) -> BockResult<()> {
        let mut args: Vec<String> = vec!["-A".to_string(), "FORWARD".to_string()];

        if let Some(src) = &rule.source {
            args.push("-s".to_string());
            args.push(src.clone());
        }

        if let Some(dst) = &rule.destination {
            args.push("-d".to_string());
            args.push(dst.clone());
        }

        if let Some(proto) = &rule.protocol {
            args.push("-p".to_string());
            args.push(proto.clone());

            if let Some(port) = rule.port {
                args.push("--dport".to_string());
                args.push(port.to_string());
            }
        }

        args.push("-j".to_string());
        args.push(rule.action.as_iptables().to_string());

        let comment = format!("bock-{}", self.container_id);
        args.push("-m".to_string());
        args.push("comment".to_string());
        args.push("--comment".to_string());
        args.push(comment);

        let output = Command::new("iptables").args(&args).output().map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to run iptables: {}", e),
            }
        })?;

        if !output.status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!(
                    "Failed to add policy rule: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        self.rules.push(rule);
        tracing::debug!(container_id = %self.container_id, "Policy rule added");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn add_rule(&mut self, _rule: PolicyRule) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "network policies".to_string(),
        })
    }

    /// Remove all rules for this container.
    #[cfg(target_os = "linux")]
    pub fn remove_all(&mut self) -> BockResult<()> {
        let comment = format!("bock-{}", self.container_id);

        // List rules and remove those with matching comment
        let list_output = Command::new("iptables")
            .args(["-L", "FORWARD", "-n", "--line-numbers"])
            .output()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to list iptables rules: {}", e),
            })?;

        // Parse line numbers for our rules (in reverse order to avoid shifting)
        let output_str = String::from_utf8_lossy(&list_output.stdout);
        let mut line_numbers: Vec<u32> = Vec::new();

        for line in output_str.lines() {
            if line.contains(&comment) {
                if let Some(num) = line.split_whitespace().next() {
                    if let Ok(n) = num.parse() {
                        line_numbers.push(n);
                    }
                }
            }
        }

        // Remove in reverse order
        line_numbers.reverse();
        for num in line_numbers {
            Command::new("iptables")
                .args(["-D", "FORWARD", &num.to_string()])
                .output()
                .ok();
        }

        self.rules.clear();
        tracing::debug!(container_id = %self.container_id, "Policy rules removed");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn remove_all(&mut self) -> BockResult<()> {
        self.rules.clear();
        Ok(())
    }

    /// Get applied rules count.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Drop for NetworkPolicy {
    fn drop(&mut self) {
        self.remove_all().ok();
    }
}

/// Allow traffic between containers on the same network.
#[cfg(target_os = "linux")]
pub fn allow_inter_container_traffic(network: &str) -> BockResult<()> {
    let output = Command::new("iptables")
        .args([
            "-A", "FORWARD", "-s", network, "-d", network, "-j", "ACCEPT",
        ])
        .output()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to run iptables: {}", e),
        })?;

    if !output.status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Failed to add inter-container rule: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    tracing::debug!(network, "Inter-container traffic allowed");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn allow_inter_container_traffic(_network: &str) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "network policies".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_rule_builder() {
        let rule = PolicyRule::allow()
            .from("10.0.0.0/8")
            .to("172.17.0.2")
            .protocol("tcp")
            .port(80);

        assert_eq!(rule.source, Some("10.0.0.0/8".to_string()));
        assert_eq!(rule.destination, Some("172.17.0.2".to_string()));
        assert_eq!(rule.port, Some(80));
        assert_eq!(rule.action, PolicyAction::Allow);
    }

    #[test]
    fn test_policy_action() {
        assert_eq!(PolicyAction::Allow.as_iptables(), "ACCEPT");
        assert_eq!(PolicyAction::Deny.as_iptables(), "REJECT");
        assert_eq!(PolicyAction::Drop.as_iptables(), "DROP");
    }
}
