#![allow(unsafe_code)]
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

        // Safety: We use CString to ensure null termination and lifetime
        let new_root_c = CString::new(new_root.to_string_lossy().as_bytes()).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Invalid path: {}", e),
            }
        })?;
        let put_old_c = CString::new(put_old.to_string_lossy().as_bytes()).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Invalid path: {}", e),
            }
        })?;

        // Perform pivot_root
        let ret = unsafe {
            libc::syscall(
                libc::SYS_pivot_root,
                new_root_c.as_ptr(),
                put_old_c.as_ptr(),
            )
        };

        if ret != 0 {
            return Err(bock_common::BockError::Io(std::io::Error::last_os_error()));
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
