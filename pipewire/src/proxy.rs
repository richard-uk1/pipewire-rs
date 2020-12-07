// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use libc::c_void;

use crate::registry::ObjectType;

#[derive(Debug)]
pub struct Proxy(*mut c_void);

// Wrapper around a proxy pointer
impl Proxy {
    pub(crate) fn new(proxy: *mut c_void) -> Self {
        Proxy(proxy)
    }

    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        unsafe {
            pw_sys::pw_proxy_destroy(self.0.cast());
        }
    }
}

// Trait implemented by high level proxy wrappers
pub trait ProxyT {
    // Add Sized restriction on those methods so it can be used as a
    // trait object, see E0038
    fn type_() -> ObjectType
    where
        Self: Sized;

    fn new(proxy: Proxy) -> Self
    where
        Self: Sized;
}

// Trait implemented by listener on high level proxy wrappers.
pub trait Listener {}
