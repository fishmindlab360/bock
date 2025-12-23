//! pivot_root implementation.

use std::path::Path;

use bock_common::BockResult;

/// Execute pivot_root to change the root filesystem.
///
/// This replaces the old root with the new root, placing the old root
/// at put_old (relative to new_root).
pub fn pivot_root(new_root: &Path, put_old: &Path) -> BockResult<()> {
    tracing::debug!(
        new_root = %new_root.display(),
        put_old = %put_old.display(),
        "Executing pivot_root"
    );

    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;

        let new_root_cstr = CString::new(new_root.to_string_lossy().as_bytes()).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Invalid path: {}", e),
            }
        })?;

        let put_old_cstr = CString::new(put_old.to_string_lossy().as_bytes()).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Invalid path: {}", e),
            }
        })?;

        // Safety: pivot_root is a standard Linux syscall
        let result = unsafe {
            libc::syscall(
                libc::SYS_pivot_root,
                new_root_cstr.as_ptr(),
                put_old_cstr.as_ptr(),
            )
        };

        if result != 0 {
            let err = std::io::Error::last_os_error();
            return Err(bock_common::BockError::Internal {
                message: format!("pivot_root failed: {}", err),
            });
        }

        tracing::debug!("pivot_root successful");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        Err(bock_common::BockError::Unsupported {
            feature: "pivot_root".to_string(),
        })
    }
}

/// Cleanup old root after pivot_root.
///
/// This unmounts and removes the old root directory.
pub fn cleanup_old_root(old_root: &Path) -> BockResult<()> {
    tracing::debug!(old_root = %old_root.display(), "Cleaning up old root");

    // Unmount recursively
    #[cfg(target_os = "linux")]
    {
        let result = unsafe {
            libc::umount2(
                old_root.to_string_lossy().as_ptr() as *const libc::c_char,
                libc::MNT_DETACH,
            )
        };

        if result != 0 {
            tracing::warn!(
                "Failed to unmount old root: {}",
                std::io::Error::last_os_error()
            );
        }
    }

    // Remove the directory
    let _ = std::fs::remove_dir_all(old_root);

    Ok(())
}
