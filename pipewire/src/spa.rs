// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

/* Re-implementation of SPA API. This should probably move to its own crate at some point */
use pipewire_sys as pw_sys;

fn list_remove(elem: &pw_sys::spa_list) {
    unsafe {
        (*elem.prev).next = elem.next;
        (*elem.next).prev = elem.prev;
    }
}

pub fn hook_remove(mut hook: pw_sys::spa_hook) {
    list_remove(&hook.link);

    if let Some(removed) = hook.removed {
        unsafe {
            removed(&mut hook as *mut _);
        }
    }
}
