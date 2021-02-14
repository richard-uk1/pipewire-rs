//! Common names for object factories.

// TODO make these `&CStr` once `CStr::from_bytes_with_nul_unchecked` is `const`.
pub const SUPPORT_LOG: &str = "support.log";
pub const SUPPORT_SYSTEM: &str = "support.system";
pub const SUPPORT_CPU: &str = "support.cpu";
pub const SUPPORT_LOOP: &str = "support.loop";
pub const SUPPORT_NODE_DRIVER: &str = "support.node.driver";
pub const SUPPORT_NULL_AUDIO_SINK: &str = "support.null-audio-sink";
