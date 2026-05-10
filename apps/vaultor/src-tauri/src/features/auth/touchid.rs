use crate::error::VaultError;

#[cfg(target_os = "macos")]
extern "C" {
    /// Defined in src/touchid.m, compiled by build.rs.
    fn vaultor_touchid_prompt(reason: *const std::os::raw::c_char) -> std::os::raw::c_int;
}

/// Show a TouchID prompt with the given reason string.
///
/// Blocks the calling thread until the user accepts or cancels.
/// Must be called from a non-main thread (Tokio spawn_blocking is correct).
pub fn prompt(reason: &str) -> Result<(), VaultError> {
    #[cfg(target_os = "macos")]
    {
        let c_reason = std::ffi::CString::new(reason).map_err(|_| VaultError::AuthFailed)?;
        let result = unsafe { vaultor_touchid_prompt(c_reason.as_ptr()) };
        if result == 1 {
            Ok(())
        } else {
            Err(VaultError::AuthFailed)
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = reason;
        // Non-macOS: always fail — TouchID is macOS-only.
        Err(VaultError::AuthFailed)
    }
}
