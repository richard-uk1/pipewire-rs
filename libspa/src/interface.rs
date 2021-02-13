// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

// FIXME: remove 'ignore' to actually build doc test once 'pipewire' crate has been updated on crates.io
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
/// ```ignore
/// use pipewire_sys as pw_sys;
/// use libspa as spa;
///
/// struct Core {
///     ptr: *mut pw_sys::pw_core
/// }
///
/// impl Core {
///     fn sync(&self, seq: i32) -> i32 {
///         unsafe {
///             spa::spa_interface_call_method!(
///                 &self.ptr, pw_sys::pw_core_methods, sync, pipewire::PW_ID_CORE, seq
///             )
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! spa_interface_call_method {
    ($interface_ptr:expr, $methods_struct:ty, $method:ident, $( $arg:expr ),*) => {{
        let iface: *mut spa_sys::spa_interface = $interface_ptr.cast();
        let funcs: *const $methods_struct = (*iface).cb.funcs.cast();
        let f = (*funcs).$method.unwrap();

        f((*iface).cb.data, $($arg),*)
    }};
}

/// Types that can be loaded as interfaces from a handle.
///
/// # Safety
///
/// See [`Interface::Type`].
pub unsafe trait Interface<'a> {
    /// The name of the interface.
    ///
    /// This should be a null-terminated string. [`Handle::interface`] will panic if this is
    /// not the case.
    ///
    /// [`Handle::interface`]: crate::Handle::interface
    // TODO move to using `&CStr` once we can create these in a `const` context.
    const NAME: &'static [u8];

    /// The version of the interface that we bind to
    const VERSION: u32;

    /// The type of the underlying interface.
    ///
    /// # Safety
    ///
    /// This must correctly correspond to [`Interface::NAME`].
    // TODO this could be split into immutable and mutable methods of the interface, but not sure
    // this is useful in practice.
    type Type;

    /// Wrap the raw interface pointer.
    ///
    /// Implementors should use PhantomData to store the lifetime. Users of the interface shouldn't
    /// have to use this function at all (use the [`Handle::interface`] method instead).
    ///
    /// [`Handle::interface`]: crate::Handle::interface
    fn from_raw(raw: &'a mut Self::Type) -> Self;
}
