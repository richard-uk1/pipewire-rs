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

/// Call a method on a spa_interface.
///
/// This needs to be called from within an `unsafe` block.
///
/// The macro always takes at least three arguments:
/// 1. A pointer to a C struct that can be casted to a spa_interface.
/// 2. The type of the interfaces methods struct.
/// 3. The name of the method that should be called.
///
/// All additional arguments are added as arguments to the call in the order they are provided.
///
/// The macro returns whatever the called method returns, for example an `i32`, or `()` if the method returns nothing.
///
/// # Examples
/// Here we call the sync method on a `pipewire_sys::pw_core` object.
/// ```rust
/// use pipewire_sys as pw_sys;
///
/// struct Core {
///     ptr: *mut pw_sys::pw_core
/// }
///
/// impl Core {
///     fn sync(&self, seq: i32) -> i32 {
///         unsafe {
///             pipewire::spa_interface_call_method!(
///                 &self.ptr, pw_sys::pw_core_methods, sync, pipewire::PW_ID_CORE, seq
///             )
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! spa_interface_call_method {
    ($interface_ptr:expr, $methods_struct:ty, $method:ident, $( $arg:expr ),*) => {{
        let iface: *mut pw_sys::spa_interface = $interface_ptr.cast();
        let funcs: *const $methods_struct = (*iface).cb.funcs.cast();
        let f = (*funcs).$method.unwrap();

        f((*iface).cb.data, $($arg),*)
    }};
}
