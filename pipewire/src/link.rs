use std::{
    ffi::{c_void, CStr},
    fmt, mem,
    pin::Pin,
};

use bitflags::bitflags;
use spa::dict::ForeignDict;

use crate::{
    proxy::{Listener, Proxy, ProxyT},
    types::ObjectType,
};

#[derive(Debug)]
pub struct Link {
    proxy: Proxy,
}

impl ProxyT for Link {
    fn type_() -> ObjectType {
        ObjectType::Link
    }

    fn upcast(self) -> Proxy {
        self.proxy
    }

    fn upcast_ref(&self) -> &Proxy {
        &self.proxy
    }

    unsafe fn from_proxy_unchecked(proxy: Proxy) -> Self
    where
        Self: Sized,
    {
        Self { proxy }
    }
}

impl Link {
    #[must_use]
    pub fn add_listener_local(&self) -> LinkListenerLocalBuilder {
        LinkListenerLocalBuilder {
            link: self,
            cbs: ListenerLocalCallbacks::default(),
        }
    }
}

pub struct LinkListener {
    // Need to stay allocated while the listener is registered
    #[allow(dead_code)]
    events: Pin<Box<pw_sys::pw_link_events>>,
    listener: Pin<Box<spa_sys::spa_hook>>,
    #[allow(dead_code)]
    data: Box<ListenerLocalCallbacks>,
}

impl<'a> Listener for LinkListener {}

impl<'a> Drop for LinkListener {
    fn drop(&mut self) {
        spa::hook::remove(*self.listener);
    }
}

#[derive(Default)]
struct ListenerLocalCallbacks {
    info: Option<Box<dyn Fn(&LinkInfo)>>,
}

pub struct LinkListenerLocalBuilder<'link> {
    link: &'link Link,
    cbs: ListenerLocalCallbacks,
}

impl<'a> LinkListenerLocalBuilder<'a> {
    #[must_use]
    pub fn info<F>(mut self, info: F) -> Self
    where
        F: Fn(&LinkInfo) + 'static,
    {
        self.cbs.info = Some(Box::new(info));
        self
    }

    #[must_use]
    pub fn register(self) -> LinkListener {
        unsafe extern "C" fn link_events_info(
            data: *mut c_void,
            info: *const pw_sys::pw_link_info,
        ) {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            let info = LinkInfo::new(info);
            callbacks.info.as_ref().unwrap()(&info);
        }

        let e = unsafe {
            let mut e: Pin<Box<pw_sys::pw_link_events>> = Box::pin(mem::zeroed());
            e.version = pw_sys::PW_VERSION_LINK_EVENTS;

            if self.cbs.info.is_some() {
                e.info = Some(link_events_info);
            }

            e
        };

        let (listener, data) = unsafe {
            let link = &self.link.proxy.as_ptr();

            let data = Box::into_raw(Box::new(self.cbs));
            let mut listener: Pin<Box<spa_sys::spa_hook>> = Box::pin(mem::zeroed());
            let listener_ptr: *mut spa_sys::spa_hook = listener.as_mut().get_unchecked_mut();
            let funcs: *const pw_sys::pw_link_events = e.as_ref().get_ref();

            pw_sys::pw_proxy_add_object_listener(
                link.cast(),
                listener_ptr.cast(),
                funcs.cast(),
                data as *mut _,
            );

            (listener, Box::from_raw(data))
        };

        LinkListener {
            events: e,
            listener,
            data,
        }
    }
}

pub struct LinkInfo {
    ptr: *const pw_sys::pw_link_info,
    props: Option<ForeignDict>,
}

impl LinkInfo {
    fn new(ptr: *const pw_sys::pw_link_info) -> Self {
        assert!(!ptr.is_null());
        let props_ptr = unsafe { (*ptr).props };
        Self {
            ptr,
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

    pub fn output_node_id(&self) -> u32 {
        unsafe { (*self.ptr).output_node_id }
    }

    pub fn output_port_id(&self) -> u32 {
        unsafe { (*self.ptr).output_port_id }
    }

    pub fn input_node_id(&self) -> u32 {
        unsafe { (*self.ptr).input_node_id }
    }

    pub fn input_port_id(&self) -> u32 {
        unsafe { (*self.ptr).input_port_id }
    }

    pub fn state(&self) -> LinkState {
        let raw_state = unsafe { (*self.ptr).state };
        match raw_state {
            pw_sys::pw_link_state_PW_LINK_STATE_ERROR => {
                let error = unsafe { CStr::from_ptr((*self.ptr).error).to_str().unwrap() };
                LinkState::Error(error)
            }
            pw_sys::pw_link_state_PW_LINK_STATE_UNLINKED => LinkState::Unlinked,
            pw_sys::pw_link_state_PW_LINK_STATE_INIT => LinkState::Init,
            pw_sys::pw_link_state_PW_LINK_STATE_NEGOTIATING => LinkState::Negotiating,
            pw_sys::pw_link_state_PW_LINK_STATE_ALLOCATING => LinkState::Allocating,
            pw_sys::pw_link_state_PW_LINK_STATE_PAUSED => LinkState::Paused,
            pw_sys::pw_link_state_PW_LINK_STATE_ACTIVE => LinkState::Active,
            _ => panic!("Invalid link state: {}", raw_state),
        }
    }

    pub fn change_mask(&self) -> LinkChangeMask {
        let mask = unsafe { (*self.ptr).change_mask };
        LinkChangeMask::from_bits(mask).expect("Invalid raw change_mask")
    }

    // TODO: format (requires SPA Pod support before it can be implemented)

    pub fn props(&self) -> Option<&ForeignDict> {
        self.props.as_ref()
    }
}

bitflags! {
    pub struct LinkChangeMask: u64 {
        const STATE = pw_sys::PW_LINK_CHANGE_MASK_STATE as u64;
        const FORMAT = pw_sys::PW_LINK_CHANGE_MASK_FORMAT as u64;
        const PROPS = pw_sys::PW_LINK_CHANGE_MASK_PROPS as u64;
    }
}

impl fmt::Debug for LinkInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkInfo")
            .field("id", &self.id())
            .field("output_node_id", &self.output_node_id())
            .field("output_portin_id", &self.output_port_id())
            .field("input_node_id", &self.input_node_id())
            .field("input_portin_id", &self.input_port_id())
            .field("change-mask", &self.change_mask())
            .field("state", &self.state())
            .field("props", &self.props())
            // TODO: .field("format", &self.format())
            .finish()
    }
}

#[derive(Debug)]
pub enum LinkState<'a> {
    Error(&'a str),
    Unlinked,
    Init,
    Negotiating,
    Allocating,
    Paused,
    Active,
}
