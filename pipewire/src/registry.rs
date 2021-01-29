// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use bitflags::bitflags;
use libc::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::mem;
use std::pin::Pin;

use crate::{
    proxy::{Proxy, ProxyT},
    types::ObjectType,
    Error,
};
use spa::dict::ForeignDict;

#[derive(Debug)]
pub struct Registry(*mut pw_sys::pw_registry);

impl Registry {
    pub(crate) fn new(registry: *mut pw_sys::pw_registry) -> Self {
        Registry(registry)
    }

    // TODO: add non-local version when we'll bind pw_thread_loop_start()
    #[must_use]
    pub fn add_listener_local(&self) -> ListenerLocalBuilder {
        ListenerLocalBuilder {
            registry: self,
            cbs: ListenerLocalCallbacks::default(),
        }
    }

    pub fn bind<T: ProxyT>(&self, object: &GlobalObject) -> Result<T, Error> {
        let proxy = unsafe {
            let type_ = CString::new(object.type_.to_str()).unwrap();
            let version = object.type_.client_version();

            let proxy = spa::spa_interface_call_method!(
                self.0,
                pw_sys::pw_registry_methods,
                bind,
                object.id,
                type_.as_ptr(),
                version,
                0
            );

            proxy
        };

        if proxy.is_null() {
            return Err(Error::NoMemory);
        }

        Proxy::new(proxy.cast()).downcast().map_err(|(_, e)| e)
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        unsafe {
            pw_sys::pw_proxy_destroy(self.0.cast());
        }
    }
}

#[derive(Default)]
struct ListenerLocalCallbacks {
    global: Option<Box<dyn Fn(GlobalObject)>>,
    global_remove: Option<Box<dyn Fn(u32)>>,
}

pub struct ListenerLocalBuilder<'a> {
    registry: &'a Registry,
    cbs: ListenerLocalCallbacks,
}

pub struct Listener {
    // Need to stay allocated while the listener is registered
    #[allow(dead_code)]
    events: Pin<Box<pw_sys::pw_registry_events>>,
    listener: Pin<Box<spa_sys::spa_hook>>,
    #[allow(dead_code)]
    data: Box<ListenerLocalCallbacks>,
}

impl<'a> Drop for Listener {
    fn drop(&mut self) {
        spa::hook::remove(*self.listener);
    }
}

impl<'a> ListenerLocalBuilder<'a> {
    #[must_use]
    pub fn global<F>(mut self, global: F) -> Self
    where
        F: Fn(GlobalObject) + 'static,
    {
        self.cbs.global = Some(Box::new(global));
        self
    }

    #[must_use]
    pub fn global_remove<F>(mut self, global_remove: F) -> Self
    where
        F: Fn(u32) + 'static,
    {
        self.cbs.global_remove = Some(Box::new(global_remove));
        self
    }

    #[must_use]
    pub fn register(self) -> Listener {
        unsafe extern "C" fn registry_events_global(
            data: *mut c_void,
            id: u32,
            permissions: u32,
            type_: *const c_char,
            version: u32,
            props: *const spa_sys::spa_dict,
        ) {
            let type_ = CStr::from_ptr(type_).to_str().unwrap();
            let obj = GlobalObject::new(id, permissions, type_, version, props);
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            callbacks.global.as_ref().unwrap()(obj);
        }

        unsafe extern "C" fn registry_events_global_remove(data: *mut c_void, id: u32) {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            callbacks.global_remove.as_ref().unwrap()(id);
        }

        let e = unsafe {
            let mut e: Pin<Box<pw_sys::pw_registry_events>> = Box::pin(mem::zeroed());
            e.version = pw_sys::PW_VERSION_REGISTRY_EVENTS;

            if self.cbs.global.is_some() {
                e.global = Some(registry_events_global);
            }
            if self.cbs.global_remove.is_some() {
                e.global_remove = Some(registry_events_global_remove);
            }

            e
        };

        let (listener, data) = unsafe {
            let ptr = self.registry.0;
            let data = Box::into_raw(Box::new(self.cbs));
            let mut listener: Pin<Box<spa_sys::spa_hook>> = Box::pin(mem::zeroed());
            let listener_ptr: *mut spa_sys::spa_hook = listener.as_mut().get_unchecked_mut();

            spa::spa_interface_call_method!(
                ptr,
                pw_sys::pw_registry_methods,
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

bitflags! {
    pub struct Permission: u32 {
        const R = pw_sys::PW_PERM_R;
        const W = pw_sys::PW_PERM_W;
        const X = pw_sys::PW_PERM_X;
        const M = pw_sys::PW_PERM_M;
    }
}

#[derive(Debug)]
pub struct GlobalObject {
    pub id: u32,
    pub permissions: Permission,
    pub type_: ObjectType,
    pub version: u32,
    pub props: Option<ForeignDict>,
}

impl GlobalObject {
    fn new(
        id: u32,
        permissions: u32,
        type_: &str,
        version: u32,
        props: *const spa_sys::spa_dict,
    ) -> Self {
        let type_ = ObjectType::from_str(type_);
        let permissions = Permission::from_bits(permissions).expect("invalid permissions");
        let props = if props.is_null() {
            None
        } else {
            Some(unsafe { ForeignDict::from_ptr(props) })
        };

        Self {
            id,
            permissions,
            type_,
            version,
            props,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_object_type() {
        assert_eq!(
            ObjectType::from_str("PipeWire:Interface:Client"),
            ObjectType::Client
        );
        assert_eq!(ObjectType::Client.to_str(), "PipeWire:Interface:Client");
        assert_eq!(ObjectType::Client.client_version(), 3);

        let o = ObjectType::Other("PipeWire:Interface:Badger".to_string());
        assert_eq!(ObjectType::from_str("PipeWire:Interface:Badger"), o);
        assert_eq!(o.to_str(), "PipeWire:Interface:Badger");
    }

    #[test]
    #[should_panic(expected = "Invalid object type")]
    fn client_version_panic() {
        let o = ObjectType::Other("PipeWire:Interface:Badger".to_string());
        assert_eq!(o.client_version(), 0);
    }
}
