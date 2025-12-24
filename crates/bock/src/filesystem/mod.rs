//! Filesystem operations for containers.
//!
//! This module handles:
//! - Root filesystem setup
//! - OverlayFS configuration
//! - Mount operations
//! - pivot_root
//! - Volume management

mod mounts;
mod overlay;
mod pivot;
mod rootfs;
mod volume;

pub use mounts::{
    MountOptions, UnmountFlags, bind_mount, make_private, make_shared, make_slave, mount,
    remount_readonly, unmount,
};
pub use overlay::OverlayFs;
pub use pivot::pivot_root;
pub use rootfs::{mount_tmpfs, setup_rootfs};
pub use volume::{Volume, VolumeManager, VolumeMount};
