// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use bitflags::bitflags;
use libc::{c_char, c_void};
use std::ffi::CStr;
use std::pin::Pin;
use std::{fmt, mem};

use crate::registry::Registry;
use spa::{dict::ForeignDict, spa_interface_call_method};

pub const PW_ID_CORE: u32 = pw_sys::PW_ID_CORE;

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
            cbs: ListenerLocalCallbacks::default(),
        }
    }

    #[must_use]
    pub fn get_registry(&self) -> Registry {
        let registry = unsafe {
            spa_interface_call_method!(
                self.0,
                pw_sys::pw_core_methods,
                get_registry,
                pw_sys::PW_VERSION_REGISTRY,
                0
            )
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
            spa_interface_call_method!(self.0, pw_sys::pw_core_methods, sync, PW_ID_CORE, seq)
        }
    }
}
#[derive(Default)]
struct ListenerLocalCallbacks {
    info: Option<Box<dyn Fn(&Info)>>,
    done: Option<Box<dyn Fn(u32, i32)>>,
    #[allow(clippy::type_complexity)]
    error: Option<Box<dyn Fn(u32, i32, i32, &str)>>, // TODO: return a proper Error enum?
                                                     // TODO: ping, remove_id, bound_id, add_mem, remove_mem
}

pub struct ListenerLocalBuilder<'a> {
    core: &'a Core,
    cbs: ListenerLocalCallbacks,
}

pub struct Listener {
    // Need to stay allocated while the listener is registered
    #[allow(dead_code)]
    events: Pin<Box<pw_sys::pw_core_events>>,
    listener: Pin<Box<spa_sys::spa_hook>>,
    #[allow(dead_code)]
    data: Box<ListenerLocalCallbacks>,
}

impl Listener {
    pub fn unregister(self) {
        // Consuming the listener will call drop()
    }
}

impl<'a> Drop for Listener {
    fn drop(&mut self) {
        spa::hook::remove(*self.listener);
    }
}

impl<'a> ListenerLocalBuilder<'a> {
    #[must_use]
    pub fn info<F>(mut self, info: F) -> Self
    where
        F: Fn(&Info) + 'static,
    {
        self.cbs.info = Some(Box::new(info));
        self
    }

    #[must_use]
    pub fn done<F>(mut self, done: F) -> Self
    where
        F: Fn(u32, i32) + 'static,
    {
        self.cbs.done = Some(Box::new(done));
        self
    }

    #[must_use]
    pub fn error<F>(mut self, error: F) -> Self
    where
        F: Fn(u32, i32, i32, &str) + 'static,
    {
        self.cbs.error = Some(Box::new(error));
        self
    }

    #[must_use]
    pub fn register(self) -> Listener {
        unsafe extern "C" fn core_events_info(
            data: *mut c_void,
            info: *const pw_sys::pw_core_info,
        ) {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            let info = Info::new(info);
            callbacks.info.as_ref().unwrap()(&info);
        }

        unsafe extern "C" fn core_events_done(data: *mut c_void, id: u32, seq: i32) {
            /* FIXME: Exposing the seq number for the user to check themselves makes the library more "low level"
               than it perhaps could be.
               See https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/merge_requests/9#note_689093
            */
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            callbacks.done.as_ref().unwrap()(id, seq);
        }

        unsafe extern "C" fn core_events_error(
            data: *mut c_void,
            id: u32,
            seq: i32,
            res: i32,
            message: *const c_char,
        ) {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            let message = CStr::from_ptr(message).to_str().unwrap();
            callbacks.error.as_ref().unwrap()(id, seq, res, message);
        }

        let e = unsafe {
            let mut e: Pin<Box<pw_sys::pw_core_events>> = Box::pin(mem::zeroed());
            e.version = pw_sys::PW_VERSION_CORE_EVENTS;

            if self.cbs.info.is_some() {
                e.info = Some(core_events_info);
            }
            if self.cbs.done.is_some() {
                e.done = Some(core_events_done);
            }
            if self.cbs.error.is_some() {
                e.error = Some(core_events_error);
            }

            e
        };

        let (listener, data) = unsafe {
            let ptr = self.core.0;
            let data = Box::into_raw(Box::new(self.cbs));
            let mut listener: Pin<Box<spa_sys::spa_hook>> = Box::pin(mem::zeroed());
            // Have to cast from pw-sys namespaced type to the equivalent spa-sys type
            // as bindgen does not allow us to generate bindings dependings of another
            // sys crate, see https://github.com/rust-lang/rust-bindgen/issues/1929
            let listener_ptr: *mut spa_sys::spa_hook = listener.as_mut().get_unchecked_mut();

            spa_interface_call_method!(
                ptr,
                pw_sys::pw_core_methods,
                add_listener,
                listener_ptr.cast(),
                e.as_ref().get_ref(),
                data as *mut _
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

pub struct Info {
    ptr: *const pw_sys::pw_core_info,
    /// Can contain a Dict wrapping the raw spa_dict at (*ptr).props.
    ///
    /// Since it is our responsibility that it does not stay alive longer than the raw dict,
    /// we store it here and only hand out borrows to it.
    props: Option<ForeignDict>,
}

impl Info {
    fn new(info: *const pw_sys::pw_core_info) -> Self {
        let props_ptr = unsafe { (*info).props };
        Self {
            ptr: info,
            props: if props_ptr.is_null() {
                None
            } else {
                Some(unsafe { ForeignDict::from_ptr(props_ptr) })
            },
        }
    }

    pub fn id(&self) -> u32 {
        unsafe { (*self.ptr).id }
    }

    pub fn cookie(&self) -> u32 {
        unsafe { (*self.ptr).cookie }
    }

    pub fn user_name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.ptr).user_name).to_str().unwrap() }
    }

    pub fn host_name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.ptr).host_name).to_str().unwrap() }
    }

    pub fn version(&self) -> &str {
        unsafe { CStr::from_ptr((*self.ptr).version).to_str().unwrap() }
    }

    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.ptr).name).to_str().unwrap() }
    }

    pub fn change_mask(&self) -> ChangeMask {
        let mask = unsafe { (*self.ptr).change_mask };
        ChangeMask::from_bits(mask).expect("invalid change_mask")
    }

    pub fn props(&self) -> Option<&ForeignDict> {
        self.props.as_ref()
    }
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
            .field("props", &self.props())
            .finish()
    }
}

bitflags! {
    pub struct ChangeMask: u64 {
        const PROPS = pw_sys::PW_CORE_CHANGE_MASK_PROPS as u64;
    }
}
