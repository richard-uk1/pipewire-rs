// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[allow(clippy::all)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
pub use bindings::*;
