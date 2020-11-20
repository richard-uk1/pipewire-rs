// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use libspa_sys as spa_sys;

use crate::list;

pub fn remove(mut hook: spa_sys::spa_hook) {
    list::remove(&hook.link);

    if let Some(removed) = hook.removed {
        unsafe {
            removed(&mut hook as *mut _);
        }
    }
}
