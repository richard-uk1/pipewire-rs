// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use bitflags::bitflags;
use libc::c_void;
use std::ffi::CStr;
use std::pin::Pin;
use std::{fmt, mem};

use crate::proxy::{Listener, Proxy, ProxyT};
use crate::registry::ObjectType;
use spa::dict::ForeignDict;

const VERSION_NODE_EVENTS: u32 = 0;

#[derive(Debug)]
pub struct Node {
    proxy: Proxy,
}

impl ProxyT for Node {
    fn type_() -> ObjectType {
        ObjectType::Node
    }

    fn new(proxy: Proxy) -> Self {
        Self { proxy }
    }
}

impl Node {
    // TODO: add non-local version when we'll bind pw_thread_loop_start()
    #[must_use]
    pub fn add_listener_local(&self) -> NodeListenerLocalBuilder {
        NodeListenerLocalBuilder {
            node: self,
            cbs: ListenerLocalCallbacks::default(),
        }
    }
}

#[derive(Default)]
struct ListenerLocalCallbacks {
    info: Option<Box<dyn Fn(&NodeInfo)>>,
    #[allow(clippy::type_complexity)]
    param: Option<Box<dyn Fn(i32, u32, u32, u32)>>, // TODO: add params
}

pub struct NodeListenerLocalBuilder<'a> {
    node: &'a Node,
    cbs: ListenerLocalCallbacks,
}

pub struct NodeInfo {
    ptr: *const pw_sys::pw_node_info,
    props: Option<ForeignDict>,
}

impl NodeInfo {
    fn new(ptr: *const pw_sys::pw_node_info) -> Self {
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

    pub fn max_input_ports(&self) -> u32 {
        unsafe { (*self.ptr).max_input_ports }
    }

    pub fn max_output_ports(&self) -> u32 {
        unsafe { (*self.ptr).max_output_ports }
    }

    pub fn change_mask(&self) -> NodeChangeMask {
        let mask = unsafe { (*self.ptr).change_mask };
        NodeChangeMask::from_bits(mask).expect("invalid change_mask")
    }

    pub fn n_input_ports(&self) -> u32 {
        unsafe { (*self.ptr).n_input_ports }
    }

    pub fn n_output_ports(&self) -> u32 {
        unsafe { (*self.ptr).n_output_ports }
    }

    pub fn state(&self) -> NodeState {
        let state = unsafe { (*self.ptr).state };
        match state {
            pw_sys::pw_node_state_PW_NODE_STATE_ERROR => {
                let error = unsafe {
                    let error = (*self.ptr).error;
                    CStr::from_ptr(error).to_str().unwrap()
                };
                NodeState::Error(error)
            }
            pw_sys::pw_node_state_PW_NODE_STATE_CREATING => NodeState::Creating,
            pw_sys::pw_node_state_PW_NODE_STATE_SUSPENDED => NodeState::Suspended,
            pw_sys::pw_node_state_PW_NODE_STATE_IDLE => NodeState::Idle,
            pw_sys::pw_node_state_PW_NODE_STATE_RUNNING => NodeState::Running,
            _ => panic!("Invalid node state: {}", state),
        }
    }

    pub fn props(&self) -> Option<&ForeignDict> {
        self.props.as_ref()
    }
    // TODO: params
}

bitflags! {
    pub struct NodeChangeMask: u64 {
        const INPUT_PORTS = (1 << 0);
        const OUTPUT_PORTS = (1 << 1);
        const STATE = (1 << 2);
        const PROPS = (1 << 3);
        const PARAMS = (1 << 4);
    }
}

#[derive(Debug)]
pub enum NodeState<'a> {
    Error(&'a str),
    Creating,
    Suspended,
    Idle,
    Running,
}

impl fmt::Debug for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeInfo")
            .field("id", &self.id())
            .field("max-input-ports", &self.max_input_ports())
            .field("max-output-ports", &self.max_output_ports())
            .field("change-mask", &self.change_mask())
            .field("n-input-ports", &self.n_input_ports())
            .field("n-output-ports", &self.n_output_ports())
            .field("state", &self.state())
            .field("props", &self.props())
            .finish()
    }
}

pub struct NodeListener {
    // Need to stay allocated while the listener is registered
    #[allow(dead_code)]
    events: Pin<Box<pw_sys::pw_node_events>>,
    listener: Pin<Box<spa_sys::spa_hook>>,
    #[allow(dead_code)]
    data: Box<ListenerLocalCallbacks>,
}

impl<'a> Listener for NodeListener {}

impl<'a> Drop for NodeListener {
    fn drop(&mut self) {
        spa::hook::remove(*self.listener);
    }
}

impl<'a> NodeListenerLocalBuilder<'a> {
    #[must_use]
    pub fn info<F>(mut self, info: F) -> Self
    where
        F: Fn(&NodeInfo) + 'static,
    {
        self.cbs.info = Some(Box::new(info));
        self
    }

    #[must_use]
    pub fn param<F>(mut self, param: F) -> Self
    where
        F: Fn(i32, u32, u32, u32) + 'static,
    {
        self.cbs.param = Some(Box::new(param));
        self
    }

    #[must_use]
    pub fn register(self) -> NodeListener {
        unsafe extern "C" fn node_events_info(
            data: *mut c_void,
            info: *const pw_sys::pw_node_info,
        ) {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            let info = NodeInfo::new(info);
            callbacks.info.as_ref().unwrap()(&info);
        }

        unsafe extern "C" fn node_events_param(
            data: *mut c_void,
            seq: i32,
            id: u32,
            index: u32,
            next: u32,
            _param: *const spa_sys::spa_pod,
        ) {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            callbacks.param.as_ref().unwrap()(seq, id, index, next);
        }

        let e = unsafe {
            let mut e: Pin<Box<pw_sys::pw_node_events>> = Box::pin(mem::zeroed());
            e.version = VERSION_NODE_EVENTS;

            if self.cbs.info.is_some() {
                e.info = Some(node_events_info);
            }
            if self.cbs.param.is_some() {
                e.param = Some(node_events_param);
            }

            e
        };

        let (listener, data) = unsafe {
            let node = &self.node.proxy.as_ptr();

            let data = Box::into_raw(Box::new(self.cbs));
            let mut listener: Pin<Box<spa_sys::spa_hook>> = Box::pin(mem::zeroed());
            let listener_ptr: *mut spa_sys::spa_hook = listener.as_mut().get_unchecked_mut();
            let funcs: *const pw_sys::pw_node_events = e.as_ref().get_ref();

            pw_sys::pw_proxy_add_object_listener(
                node.cast(),
                listener_ptr.cast(),
                funcs.cast(),
                data as *mut _,
            );

            (listener, Box::from_raw(data))
        };

        NodeListener {
            events: e,
            listener,
            data,
        }
    }
}
