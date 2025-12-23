//! Filesystem operations for containers.
//!
//! This module handles:
//! - Root filesystem setup
//! - OverlayFS configuration
//! - Mount operations
//! - pivot_root

mod mounts;
mod overlay;
mod pivot;
mod rootfs;

pub use mounts::{mount, unmount, MountOptions};
pub use overlay::OverlayFs;
pub use pivot::pivot_root;
pub use rootfs::setup_rootfs;
