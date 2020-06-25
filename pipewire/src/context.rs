// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use pipewire_sys as pw_sys;
use std::ptr;

use crate::core_::Core;
use crate::error::Error;
use crate::loop_::Loop;

#[derive(Debug)]
pub struct Context(*mut pw_sys::pw_context);

impl Context {
    // TODO: properties argument
    pub fn new(loop_: &Loop) -> Result<Self, Error> {
        unsafe {
            let context = pw_sys::pw_context_new(loop_.to_ptr(), ptr::null_mut(), 0);
            if context.is_null() {
                Err(Error::CreationFailed)
            } else {
                Ok(Context(context))
            }
        }
    }

    // TODO: properties argument
    pub fn connect(&self) -> Result<Core, Error> {
        unsafe {
            let core = pw_sys::pw_context_connect(self.0, ptr::null_mut(), 0);
            if core.is_null() {
                // TODO: check errno to set better error
                Err(Error::CreationFailed)
            } else {
                Ok(Core::from_ptr(core))
            }
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { pw_sys::pw_context_destroy(self.0) }
    }
}
