// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init() {
        unsafe {
            pw_init(std::ptr::null_mut(), std::ptr::null_mut());
            pw_deinit();
        }
    }
}
