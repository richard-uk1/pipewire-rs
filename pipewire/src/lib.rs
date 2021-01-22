// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::ptr;

mod error;
pub use error::*;
pub mod loop_;
pub use loop_::*;
mod main_loop;
pub use main_loop::*;
mod context;
pub use context::*;
mod core_;
pub use core_::*;
mod properties;
pub use properties::*;
pub mod link;
pub mod node;
pub mod port;
pub mod proxy;
pub mod registry;
pub use spa;
mod utils;

// Re-export all the traits in a prelude module, so that applications
// can always "use pipewire::prelude::*" without getting conflicts
pub mod prelude {
    pub use crate::loop_::Loop;
}

/// Initialize PipeWire
///
/// Initialize the PipeWire system and set up debugging
/// through the environment variable `PIPEWIRE_DEBUG`.
pub fn init() {
    unsafe { pw_sys::pw_init(ptr::null_mut(), ptr::null_mut()) }
}

/// Deinitialize PipeWire
///
/// # Safety
/// This must only be called once during the lifetime of the process, once no PipeWire threads
/// are running anymore and all PipeWire resources are released.
pub unsafe fn deinit() {
    pw_sys::pw_deinit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
        unsafe {
            deinit();
        }
    }
}
