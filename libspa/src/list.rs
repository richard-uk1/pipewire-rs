// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use libspa_sys as spa_sys;

pub fn remove(elem: &spa_sys::spa_list) {
    unsafe {
        (*elem.prev).next = elem.next;
        (*elem.next).prev = elem.prev;
    }
}
