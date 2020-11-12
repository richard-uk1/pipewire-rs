// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use pipewire_sys as pw_sys;
use std::ptr;

use crate::error::Error;
use crate::loop_::Loop;

#[derive(Debug)]
pub struct MainLoop(*mut pw_sys::pw_main_loop);

impl MainLoop {
    // TODO: props argument
    pub fn new() -> Result<Self, Error> {
        unsafe {
            let l = pw_sys::pw_main_loop_new(ptr::null());
            if l.is_null() {
                Err(Error::CreationFailed)
            } else {
                Ok(MainLoop(l))
            }
        }
    }

    pub fn run(&self) {
        unsafe {
            pw_sys::pw_main_loop_run(self.0);
        }
    }

    pub fn quit(&self) {
        unsafe {
            pw_sys::pw_main_loop_quit(self.0);
        }
    }
}

impl Loop for MainLoop {
    fn as_ptr(&self) -> *mut pw_sys::pw_loop {
        unsafe { pw_sys::pw_main_loop_get_loop(self.0) }
    }
}

impl Drop for MainLoop {
    fn drop(&mut self) {
        unsafe { pw_sys::pw_main_loop_destroy(self.0) }
    }
}
