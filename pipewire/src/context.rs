// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::ptr;

use crate::core_::Core;
use crate::error::Error;
use crate::loop_::Loop;
use crate::properties::Properties;

#[derive(Debug)]
pub struct Context<T: Loop + Clone>(*mut pw_sys::pw_context, T);

impl<T: Loop + Clone> Context<T> {
    // TODO: properties argument
    pub fn new(loop_: &T) -> Result<Self, Error> {
        unsafe {
            let context = pw_sys::pw_context_new(loop_.as_ptr(), ptr::null_mut(), 0);
            if context.is_null() {
                Err(Error::CreationFailed)
            } else {
                Ok(Context(context, loop_.clone()))
            }
        }
    }

    pub fn connect(&self, properties: Option<Properties>) -> Result<Core, Error> {
        let properties = properties.map_or(ptr::null_mut(), |p| p.into_raw());

        unsafe {
            let core = pw_sys::pw_context_connect(self.0, properties, 0);
            if core.is_null() {
                // TODO: check errno to set better error
                Err(Error::CreationFailed)
            } else {
                Ok(Core::from_ptr(core))
            }
        }
    }
}

impl<T: Loop + Clone> Drop for Context<T> {
    fn drop(&mut self) {
        unsafe { pw_sys::pw_context_destroy(self.0) }
    }
}
