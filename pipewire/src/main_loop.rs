// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use pipewire_sys as pw_sys;
use std::ops::Deref;
use std::ptr;
use std::rc::Rc;

use crate::error::Error;
use crate::loop_::Loop;

#[derive(Debug, Clone)]
pub struct MainLoop {
    inner: Rc<MainLoopInner>,
}

impl MainLoop {
    pub fn new() -> Result<Self, Error> {
        let inner = MainLoopInner::new()?;
        Ok(Self {
            inner: Rc::new(inner),
        })
    }
}

impl Deref for MainLoop {
    type Target = MainLoopInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Loop for MainLoop {
    fn as_ptr(&self) -> *mut pw_sys::pw_loop {
        unsafe { pw_sys::pw_main_loop_get_loop(self.inner.0) }
    }
}

#[derive(Debug)]
pub struct MainLoopInner(*mut pw_sys::pw_main_loop);

impl MainLoopInner {
    // TODO: props argument
    pub fn new() -> Result<Self, Error> {
        unsafe {
            let l = pw_sys::pw_main_loop_new(ptr::null());
            if l.is_null() {
                Err(Error::CreationFailed)
            } else {
                Ok(MainLoopInner(l))
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

impl Drop for MainLoopInner {
    fn drop(&mut self) {
        unsafe { pw_sys::pw_main_loop_destroy(self.0) }
    }
}
