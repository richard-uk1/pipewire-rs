// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use pipewire_sys as pw_sys;

#[derive(Debug)]
pub struct Core(*mut pw_sys::pw_core);

impl Core {
    pub(crate) fn from_ptr(core: *mut pw_sys::pw_core) -> Self {
        Core(core)
    }
}
