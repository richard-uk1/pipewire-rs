// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use bitflags::bitflags;
use libc::{c_char, c_void};
use std::ffi::CStr;
use std::pin::Pin;
use std::{fmt, mem};

use crate::registry::Registry;
use crate::spa;
use pipewire_sys as pw_sys;

const VERSION_CORE_EVENTS: u32 = 0;
const PW_VERSION_REGISTRY: u32 = 3;
const PW_ID_CORE: u32 = 0;

#[derive(Debug)]
pub struct Core(*mut pw_sys::pw_core);

impl Core {
    pub(crate) fn from_ptr(core: *mut pw_sys::pw_core) -> Self {
        Core(core)
    }

    // TODO: add non-local version when we'll bind pw_thread_loop_start()
    #[must_use]
    pub fn add_listener_local(&self) -> ListenerLocalBuilder {
        ListenerLocalBuilder {
            core: self,
            info: None,
            done: None,
            error: None,
        }
    }

    #[must_use]
    pub fn get_registry(&self) -> Registry {
        let registry = unsafe {
            let iface: *mut pw_sys::spa_interface = self.0.cast();
            let funcs: *const pw_sys::pw_core_methods = (*iface).cb.funcs.cast();
            let f = (*funcs).get_registry.unwrap();

            f((*iface).cb.data, PW_VERSION_REGISTRY, 0)
        };

        Registry::new(registry)
    }

    /* FIXME: Return type is a SPA Result as seen here:
       https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/doc/spa/design.md#error-codes.
       A type that represents this more idomatically should be returned.
       See also: https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/merge_requests/9#note_689093
    */
    pub fn sync(&self, seq: i32) -> i32 {
        unsafe {
            let iface: *mut pw_sys::spa_interface = self.0.cast();
            let funcs: *const pw_sys::pw_core_methods = (*iface).cb.funcs.cast();
            let f = (*funcs).sync.unwrap();

            let res = f((*iface).cb.data, PW_ID_CORE, seq);

            res as i32
        }
    }
}

pub struct ListenerLocalBuilder<'a> {
    core: &'a Core,
    info: Option<Box<dyn Fn(&Info)>>,
    done: Option<Box<dyn Fn(u32, i32)>>,
    #[allow(clippy::type_complexity)]
    error: Option<Box<dyn Fn(u32, i32, i32, &str)>>, // TODO: return a proper Error enum?
                                                     // TODO: ping, remove_id, bound_id, add_mem, remove_mem
}

pub struct Listener<'a> {
    // Need to stay allocated while the listener is registered
    #[allow(dead_code)]
    events: Pin<Box<pw_sys::pw_core_events>>,
    listener: Pin<Box<pipewire_sys::spa_hook>>,
    #[allow(dead_code)]
    data: Box<ListenerLocalBuilder<'a>>,
}

impl<'a> Drop for Listener<'a> {
    fn drop(&mut self) {
        spa::hook_remove(*self.listener);
    }
}

impl<'a> ListenerLocalBuilder<'a> {
    #[must_use]
    pub fn info<F>(mut self, info: F) -> Self
    where
        F: Fn(&Info) + 'static,
    {
        self.info = Some(Box::new(info));
        self
    }

    #[must_use]
    pub fn done<F>(mut self, done: F) -> Self
    where
        F: Fn(u32, i32) + 'static,
    {
        self.done = Some(Box::new(done));
        self
    }

    #[must_use]
    pub fn error<F>(mut self, error: F) -> Self
    where
        F: Fn(u32, i32, i32, &str) + 'static,
    {
        self.error = Some(Box::new(error));
        self
    }

    #[must_use]
    pub fn register(self) -> Listener<'a> {
        unsafe extern "C" fn core_events_info(
            data: *mut c_void,
            info: *const pw_sys::pw_core_info,
        ) {
            let callbacks = (data as *mut ListenerLocalBuilder).as_ref().unwrap();
            let info = Info::new(info);
            callbacks.info.as_ref().unwrap()(&info);
        }

        unsafe extern "C" fn core_events_done(data: *mut c_void, id: u32, seq: i32) {
            /* FIXME: Exposing the seq number for the user to check themselves makes the library more "low level"
               than it perhaps could be.
               See https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/merge_requests/9#note_689093
            */
            let callbacks = (data as *mut ListenerLocalBuilder).as_ref().unwrap();
            callbacks.done.as_ref().unwrap()(id, seq);
        }

        unsafe extern "C" fn core_events_error(
            data: *mut c_void,
            id: u32,
            seq: i32,
            res: i32,
            message: *const c_char,
        ) {
            let callbacks = (data as *mut ListenerLocalBuilder).as_ref().unwrap();
            let message = CStr::from_ptr(message).to_str().unwrap();
            callbacks.error.as_ref().unwrap()(id, seq, res, message);
        }

        let e = unsafe {
            let mut e: Pin<Box<pw_sys::pw_core_events>> = Box::pin(mem::zeroed());
            e.version = VERSION_CORE_EVENTS;

            if self.info.is_some() {
                e.info = Some(core_events_info);
            }
            if self.done.is_some() {
                e.done = Some(core_events_done);
            }
            if self.error.is_some() {
                e.error = Some(core_events_error);
            }

            e
        };

        let (listener, data) = unsafe {
            let iface: *mut pw_sys::spa_interface = self.core.0.cast();
            let funcs: *const pw_sys::pw_core_methods = (*iface).cb.funcs.cast();
            let f = (*funcs).add_listener.unwrap();

            let data = Box::into_raw(Box::new(self));
            let mut listener: Pin<Box<pw_sys::spa_hook>> = Box::pin(mem::zeroed());

            f(
                (*iface).cb.data,
                listener.as_mut().get_unchecked_mut(),
                e.as_ref().get_ref(),
                data as *mut _,
            );

            (listener, Box::from_raw(data))
        };

        Listener {
            events: e,
            listener,
            data,
        }
    }
}

pub struct Info(*const pw_sys::pw_core_info);

impl Info {
    fn new(info: *const pw_sys::pw_core_info) -> Self {
        Self(info)
    }

    pub fn id(&self) -> u32 {
        unsafe { (*self.0).id }
    }

    pub fn cookie(&self) -> u32 {
        unsafe { (*self.0).cookie }
    }

    pub fn user_name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.0).user_name).to_str().unwrap() }
    }

    pub fn host_name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.0).host_name).to_str().unwrap() }
    }

    pub fn version(&self) -> &str {
        unsafe { CStr::from_ptr((*self.0).version).to_str().unwrap() }
    }

    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.0).name).to_str().unwrap() }
    }

    pub fn change_mask(&self) -> ChangeMask {
        let mask = unsafe { (*self.0).change_mask };
        ChangeMask::from_bits(mask).expect("invalid change_mask")
    }

    // TODO: props
}

impl fmt::Debug for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreInfo")
            .field("id", &self.id())
            .field("cookie", &self.cookie())
            .field("user-name", &self.user_name())
            .field("host-name", &self.host_name())
            .field("version", &self.version())
            .field("name", &self.name())
            .field("change-mask", &self.change_mask())
            .finish()
    }
}

bitflags! {
    pub struct ChangeMask: u64 {
        const PROPS = (1 << 0);
    }
}
