//! # Bock Container Runtime
//!
//! Bock is a modern, OCI-compliant container runtime built in Rust.
//!
//! ## Features
//!
//! - **Namespace isolation**: Full support for Linux namespaces (user, pid, net, mount, uts, ipc, cgroup)
//! - **Cgroup v2**: Modern cgroup management for resource limits
//! - **Security**: Seccomp, capabilities, AppArmor support
//! - **OCI compliance**: Full OCI runtime specification support
//!
//! ## Usage
//!
//! ```no_run
//! use bock::runtime::Container;
//! use bock_oci::Spec;
//!
//! # async fn example() -> bock_common::BockResult<()> {
//! // Load OCI spec
//! let spec = Spec::default();
//!
//! // Create container
//! let container = Container::create("my-container", &spec).await?;
//!
//! // Start container
//! container.start().await?;
//!
//! // Wait for exit
//! let exit_code = container.wait().await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod cgroup;
pub mod cli;
pub mod exec;
pub mod filesystem;
pub mod namespace;

pub mod runtime;
pub mod security;

pub use runtime::Container;
