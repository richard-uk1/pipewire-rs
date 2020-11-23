// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use libc::{c_int, c_void};
use libspa::spa_interface_call_method;
use libspa_sys as spa_sys;
use pipewire_sys as pw_sys;
use signal::Signal;

use crate::utils::assert_main_thread;

pub trait Loop {
    fn as_ptr(&self) -> *mut pw_sys::pw_loop;

    #[must_use]
    fn add_signal_local<F>(&self, signal: Signal, callback: F) -> Source<F, Self>
    where
        F: Fn() + 'static,
        Self: Sized,
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
            let mut iface = self
                .as_ptr()
                .as_ref()
                .unwrap()
                .utils
                .as_ref()
                .unwrap()
                .iface;

            let source = spa_interface_call_method!(
                &mut iface as *mut pw_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                add_signal,
                signal as c_int,
                Some(call_closure::<F>),
                data as *mut _
            );

            (source, Box::from_raw(data))
        };

        Source {
            source,
            loop_: &self,
            data,
        }
    }

    fn destroy_source<F>(&self, source: &Source<F, Self>)
    where
        F: Fn() + 'static,
        Self: Sized,
    {
        unsafe {
            let mut iface = self
                .as_ptr()
                .as_ref()
                .unwrap()
                .utils
                .as_ref()
                .unwrap()
                .iface;

            spa_interface_call_method!(
                &mut iface as *mut pw_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                destroy_source,
                source.source
            )
        }
    }
}
pub struct Source<'a, F, L>
where
    F: Fn() + 'static,
    L: Loop,
{
    source: *mut spa_sys::spa_source,
    loop_: &'a L,
    // Store data wrapper to prevent leak
    #[allow(dead_code)]
    data: Box<F>,
}

impl<'a, F, L> Drop for Source<'a, F, L>
where
    F: Fn() + 'static,
    L: Loop,
{
    fn drop(&mut self) {
        self.loop_.destroy_source(&self)
    }
}
