// Copyright 2020, Collabora Ltd.
// Licensed under the MIT license, see the LICENSE file or <https://opensource.org/licenses/MIT>

use std::ptr;

use pipewire_sys as pw_sys;

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
