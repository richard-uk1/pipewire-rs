// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use pipewire_sys as pw_sys;

#[derive(Debug)]
pub struct Registry(*mut pw_sys::pw_registry);

impl Registry {
    pub(crate) fn new(registry: *mut pw_sys::pw_registry) -> Self {
        Registry(registry)
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        unsafe {
            pw_sys::pw_proxy_destroy(self.0.cast());
        }
    }
}
