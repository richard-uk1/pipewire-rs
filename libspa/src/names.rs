//! Common names for object factories.

// TODO make these `&CStr` once `CStr::from_bytes_with_nul_unchecked` is `const`.
pub const SUPPORT_LOG: &str = "support.log";
pub const SUPPORT_CPU: &str = "support.cpu";
