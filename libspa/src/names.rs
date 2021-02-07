//! Common names for interface factories.

// TODO make these `&CStr` once `CStr::from_bytes_with_nul_unchecked` is `const`.
pub const SUPPORT_LOG: &[u8] = spa_sys::SPA_NAME_SUPPORT_LOG;
