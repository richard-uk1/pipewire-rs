// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use libc::{c_int, c_void};
use pipewire_sys as pw_sys;
use signal::Signal;
use std::ptr;

use crate::error::Error;
use crate::utils::assert_main_thread;

#[derive(Debug)]
pub struct Loop(*mut pw_sys::pw_loop, bool);

impl Loop {
    pub(crate) fn from_ptr(l: *mut pw_sys::pw_loop, owned: bool) -> Self {
        Loop(l, owned)
    }

    // TODO: props argument
    pub fn new() -> Result<Self, Error> {
        unsafe {
            let l = pw_sys::pw_loop_new(ptr::null());
            if l.is_null() {
                Err(Error::CreationFailed)
            } else {
                Ok(Loop::from_ptr(l, true))
            }
        }
    }

    pub(crate) fn to_ptr(&self) -> *mut pw_sys::pw_loop {
        self.0
    }

    #[must_use]
    pub fn add_signal_local<F>(&self, signal: Signal, callback: F) -> Source<F>
    where
        F: Fn() + 'static,
    {
        assert_main_thread();

        unsafe extern "C" fn call_closure<F>(data: *mut c_void, _signal: c_int)
        where
            F: Fn(),
        {
            let callback = (data as *mut F).as_ref().unwrap();
            callback();
        }

        let data = Box::into_raw(Box::new(callback));

        let (source, data) = unsafe {
            let iface = self.0.as_ref().unwrap().utils.as_ref().unwrap().iface;
            let funcs: *const pw_sys::spa_loop_utils_methods = iface.cb.funcs.cast();
            let f = (*funcs).add_signal.unwrap();

            let source = f(
                iface.cb.data,
                signal as c_int,
                Some(call_closure::<F>),
                data as *mut _,
            );

            (source, Box::from_raw(data))
        };

        Source {
            source,
            loop_: &self,
            data,
        }
    }

    fn destroy_source<F>(&self, source: &Source<F>)
    where
        F: Fn() + 'static,
    {
        unsafe {
            let iface = self.0.as_ref().unwrap().utils.as_ref().unwrap().iface;
            let funcs: *const pw_sys::spa_loop_utils_methods = iface.cb.funcs.cast();
            let f = (*funcs).destroy_source.unwrap();

            f(iface.cb.data, source.source)
        }
    }
}

impl Drop for Loop {
    fn drop(&mut self) {
        if self.1 {
            unsafe { pw_sys::pw_loop_destroy(self.0) }
        }
    }
}

pub struct Source<'a, F>
where
    F: Fn() + 'static,
{
    source: *mut pw_sys::spa_source,
    loop_: &'a Loop,
    // Store data wrapper to prevent leak
    #[allow(dead_code)]
    data: Box<F>,
}

impl<'a, F> Drop for Source<'a, F>
where
    F: Fn() + 'static,
{
    fn drop(&mut self) {
        self.loop_.destroy_source(&self)
    }
}
