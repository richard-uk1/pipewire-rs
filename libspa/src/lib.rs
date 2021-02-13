// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT
use crate::interface::Interface;
use anyhow::Error;
use libloading::{Library, Symbol};
use spa_sys::{
    spa_handle, spa_handle_factory, spa_interface, spa_interface_info,
    SPA_HANDLE_FACTORY_ENUM_FUNC_NAME,
};
use std::{
    alloc,
    borrow::Cow,
    convert::TryInto,
    ffi::CStr,
    fmt, io, mem,
    mem::align_of,
    os::raw::{c_int, c_void},
    path::Path,
    ptr,
    rc::Rc,
};

pub mod dict;
pub mod hook;
pub mod interface;
pub mod list;
pub mod names;
pub mod support;

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

const SPA_ROOT: &str = "/usr/lib64/spa-0.2";

/// A libspa plugin, loaded from a shared library.
pub struct Plugin {
    /// A RAII handle to the shared library containing the plugin.
    // Object handles will all share ownership of the library, so it lives at least as long as
    // the handles.
    lib: Rc<Library>,
}

impl fmt::Debug for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct DebugFactories<'a>(&'a Plugin);
        impl fmt::Debug for DebugFactories<'_> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_list().entries(self.0.factories()).finish()
            }
        }

        f.debug_struct("Plugin")
            .field("factories", &DebugFactories(self))
            .finish()
    }
}

impl Plugin {
    /// Open a plugin.
    ///
    /// Search for a plugin at `path` in the default plugin directory.
    #[inline]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = Path::new(SPA_ROOT).join(path.as_ref());
        Self::open_absolute_path(&path)
    }

    /// Open the plugin at `path`.
    ///
    /// Path is either absolute, or relative to the current working directory of the process this
    /// function was called from.
    pub fn open_absolute_path(path: impl AsRef<Path>) -> Result<Self> {
        unsafe {
            let lib = Library::new(path.as_ref())?;
            let plugin = Plugin { lib: Rc::new(lib) };
            // Check we can load the factory enum function
            plugin.enum_fn()?;
            Ok(plugin)
        }
    }

    /// Load the enum function from the library.
    ///
    /// We don't use the typedef in *sys because it wrapped in `Option` for correctness. We don't
    /// need to think about a nullable function pointer because libloading handles it for us.
    ///
    /// Strictly speaking this function is unsafe, if the enum function has the wrong type
    /// signature. Since this is the fault of the plugin creator rather than us, we consider this
    /// function safe.
    fn enum_fn(
        &self,
    ) -> Result<
        Symbol<
            unsafe extern "C" fn(factory: *mut *const spa_handle_factory, index: *mut u32) -> c_int,
        >,
    > {
        unsafe {
            self.lib
                .get(SPA_HANDLE_FACTORY_ENUM_FUNC_NAME)
                .map_err(Into::into)
        }
    }

    /// Get an iterator over all the factories.
    pub fn factories(&self) -> FactoryIter {
        FactoryIter::new(self)
    }

    /// Get a factory by name. Equivalent to
    ///
    /// ```ignore
    /// self.factories().find(|f| f.name() == name)
    /// ```
    pub fn factory<'a>(&'a self, name: &str) -> Option<Factory<'a>> {
        self.factories().find(|factory| factory.name() == name)
    }
}

/// An iterator over the factories available in the plugin.
pub struct FactoryIter<'a> {
    plugin: &'a Plugin,
    index: u32,
}

impl<'a> FactoryIter<'a> {
    fn new(plugin: &'a Plugin) -> Self {
        FactoryIter { plugin, index: 0 }
    }
}

impl<'a> Iterator for FactoryIter<'a> {
    type Item = Factory<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // We already checked this symbol exists when creating the plugin object, so just panic
        // here if there is an issue.
        let enum_fn = self.plugin.enum_fn().unwrap();
        let mut factory: *const spa_handle_factory = ptr::null();
        // There really shouldn't be any errors here, so we convert them to panics.
        let ret = unsafe {
            SpaResult::from_raw(enum_fn(&mut factory, &mut self.index))
                .into_sync_result()
                .unwrap()
        };
        if ret == 0 {
            // signals end of factories enumeration.
            None
        } else {
            // Safety: lifetime of factory is tied to plugin.
            unsafe { Some(Factory::new(&*factory, self.plugin)) }
        }
    }
}

/// A factory that can create objects and return handles to them.
pub struct Factory<'a> {
    raw: &'a spa_handle_factory,
    plugin: &'a Plugin,
}

impl fmt::Debug for Factory<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct DebugInterfaces<'a>(&'a Factory<'a>);
        impl fmt::Debug for DebugInterfaces<'_> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_list().entries(self.0.interfaces()).finish()
            }
        }

        f.debug_struct("Factory")
            .field("version", &self.version())
            .field("name", &self.name())
            .field("interfaces", &DebugInterfaces(self))
            .finish()
    }
}

impl<'a> Factory<'a> {
    fn new(raw: &'a spa_handle_factory, plugin: &'a Plugin) -> Self {
        Factory { raw, plugin }
    }

    /// The version of the factory.
    pub fn version(&self) -> u32 {
        self.raw.version
    }

    /// Get the name of the factory.
    pub fn name(&self) -> Cow<'a, str> {
        unsafe { CStr::from_ptr(self.raw.name).to_string_lossy() }
    }

    /// The size required to store an object from this factory.
    fn size(&self) -> usize {
        unsafe {
            // Converting from u32 to ptr width should never fail (would require use of a 16 bit
            // machine and reasonably large object, >65kB)
            (self.raw.get_size.unwrap())(self.raw, ptr::null())
                .try_into()
                .unwrap()
        }
    }

    /// The memory layout of objects from this factory.
    fn layout(&self) -> alloc::Layout {
        // Copy behaviour of malloc by forcing largest possible alignment.
        // `libspa` actually provides a method `get_max_align` on `spa_cpu`, but we can't use it
        // because of the chicken/egg problem.
        alloc::Layout::from_size_align(self.size(), align_of::<libc::max_align_t>()).unwrap()
    }

    pub fn interfaces(&self) -> InterfaceInfoIter {
        InterfaceInfoIter::new(self)
    }

    /// Instantiate an instance of the object this factory creates.
    ///
    /// The handle will own a reference to the shared library, allowing the object to be used even
    /// if the `plugin` is dropped.
    pub fn instantiate(&self) -> Handle {
        unsafe {
            let layout = self.layout();
            let handle = alloc::alloc_zeroed(layout) as *mut spa_handle;
            let ret = SpaResult::from_raw((self.raw.init.unwrap())(
                self.raw as *const _,
                handle,
                ptr::null(),
                ptr::null(),
                0,
            ))
            .into_sync_result();
            if let Err(e) = ret {
                alloc::dealloc(handle as *mut u8, layout);
                // TODO handle error (return Result)
                panic!("init failed: {}", e);
            }
            Handle {
                lib: self.plugin.lib.clone(),
                handle: Rc::new(RawHandle {
                    size: self.size(),
                    inner: handle,
                }),
            }
        }
    }
}

/// An iterator over information on the interfaces for a factory.
pub struct InterfaceInfoIter<'a> {
    factory: &'a Factory<'a>,
    index: u32,
}

impl<'a> InterfaceInfoIter<'a> {
    fn new(factory: &'a Factory) -> Self {
        Self { factory, index: 0 }
    }
}

impl<'a> Iterator for InterfaceInfoIter<'a> {
    type Item = InterfaceInfo<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut interface: *const spa_interface_info = ptr::null();
            let ret = SpaResult::from_raw(((*self.factory.raw).enum_interface_info.unwrap())(
                self.factory.raw,
                &mut interface,
                &mut self.index,
            ))
            .into_sync_result()
            .unwrap();
            if ret == 0 {
                None
            } else {
                // Safety: lifetime of object is tied to self, so ref is always valid.
                Some(InterfaceInfo::new(&*interface))
            }
        }
    }
}

/// Info about an interface to an object.
pub struct InterfaceInfo<'a> {
    raw: &'a spa_interface_info,
}

impl fmt::Debug for InterfaceInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InterfaceInfo")
            .field("type", &self.type_())
            .finish()
    }
}

impl<'a> InterfaceInfo<'a> {
    fn type_(&self) -> Cow<'a, str> {
        unsafe { CStr::from_ptr(self.raw.type_).to_string_lossy() }
    }
}

impl<'a> InterfaceInfo<'a> {
    /// # Safety
    ///
    /// The user must ensure `raw` lives at least as long as `'a`.
    unsafe fn new(raw: &'a spa_interface_info) -> Self {
        InterfaceInfo { raw }
    }
}

/// A handle to an object instantiated from one of the plugin factories.
///
/// This object is untyped. To be useful we need to know what kind of object this is a handle for.
/// I need to think more about the best way to do this. Since we keep a handle to the library, we
/// could also store pointers to the name and version of the factory, if that were useful.
pub struct Handle {
    // There is an implicit dependency of `handle` on `lib`.
    #[allow(dead_code)]
    lib: Rc<Library>,
    handle: Rc<RawHandle>,
}

impl Handle {
    /// Clear up after the handle.
    ///
    /// Equivalent to dropping the handle, but in addition will report errors.
    pub fn clear(self) -> io::Result<()> {
        let Handle { lib: _, handle } = self;
        if let Ok(handle) = Rc::try_unwrap(handle) {
            handle.clear()
        } else {
            Ok(())
        }
    }

    /// Get an interface from the factory handle.
    ///
    /// This function borrows the handle to ensure that the handle lives at least as long as the
    /// interface is in use.
    ///
    /// Returns `None` if the interface is not present
    pub fn interface<'a, T: 'a + Interface<'a>>(&'a mut self) -> Option<T> {
        let name = CStr::from_bytes_with_nul(T::NAME).unwrap();
        let mut iface: *mut c_void = ptr::null_mut();
        unsafe {
            if let Err(e) = SpaResult::from_raw(((*self.handle.inner).get_interface.unwrap())(
                self.handle.inner,
                name.as_ptr(),
                &mut iface,
            ))
            .into_sync_result()
            {
                match e.raw_os_error() {
                    Some(libc::ENOTSUP) => return None,
                    _ => panic!(e),
                }
            }
            // Safety: the first field of an interface is `spa_interface`, so we can reinterpret.
            let generic_iface = iface.cast::<spa_interface>();
            let version = (*generic_iface).version;
            if version != T::VERSION {
                return None;
            }
            // Safety: iface points to a valid object with lifetime 'a.
            Some(T::from_raw(&mut *iface.cast()))
        }
    }
}

struct RawHandle {
    /// Memory we allocated for the handle.
    size: usize,
    inner: *mut spa_handle,
}

impl RawHandle {
    /// Clean up the handle.
    ///
    /// Same as the implicit `Drop`, but reports errors.
    fn clear(self) -> io::Result<()> {
        let mem_layout =
            alloc::Layout::from_size_align(self.size, align_of::<libc::max_align_t>()).unwrap();
        unsafe {
            let ret = (*self.inner).clear.unwrap()(self.inner);
            alloc::dealloc(self.inner as *mut u8, mem_layout);
            mem::forget(self);
            SpaResult::from_raw(ret).into_sync_result().map(|_| ())
        }
    }
}

impl Drop for RawHandle {
    fn drop(&mut self) {
        let mem_layout =
            alloc::Layout::from_size_align(self.size, align_of::<libc::max_align_t>()).unwrap();
        unsafe {
            (*self.inner).clear.unwrap()(self.inner);
            alloc::dealloc(self.inner as *mut u8, mem_layout);
        }
    }
}

/*
/// An interface to an object.
///
/// /
pub struct Interface<'a, T> {
    handle: Rc<RawHandle>,
    inner: *mut T,
    lifetime: PhantomData<&'a ()>,
}
*/

#[derive(Debug, Copy, Clone)]
pub struct SpaResult(i32);
/*
    /// Function successfully completed.
    Sync {
        /// The returned value.
        ///
        /// The meaning is function-specific.
        val: i32,
    },
    /// Function is running and will return asynchronously.
    Async {
        /// The sequence number of the async result.
        ///
        /// This can be used to identify the result later. It should be considered an opaque value.
        seq: i32,
    },
}
*/

// Top bit is sign, next bit is async flag, everything else is the value/sequence number.
const SPA_ASYNC_BIT: i32 = 1 << 30; // last bit before sign.
const SPA_VAL_MASK: i32 = SPA_ASYNC_BIT - 1;

impl SpaResult {
    pub fn from_raw(res: i32) -> Self {
        Self(res)
    }

    pub fn into_raw(self) -> i32 {
        self.0
    }

    pub fn new_ok(val: i32) -> Self {
        if (val & !SPA_VAL_MASK) != 0 {
            panic!("val must be positive number <= 2147483647");
        }
        SpaResult(val)
    }

    pub fn new_async(seq: i32) -> Self {
        if (seq & !SPA_VAL_MASK) != 0 {
            panic!("seq must be positive number <= 2147483647");
        }
        SpaResult(seq | SPA_ASYNC_BIT)
    }

    pub fn new_err(code: i32) -> Self {
        if code <= 0 {
            panic!("error code must be > 0");
        }
        SpaResult(-code)
    }

    /// Unwrap the sync output value, panicking if the result was asyncronous or an error.
    pub fn unwrap_ok(self) -> i32 {
        if (self.0 & !SPA_VAL_MASK) != 0 {
            panic!("value was not a synchronous success");
        }
        self.0
    }

    /// Unwrap the async sequence number, panicking if the result was syncronous or an error.
    pub fn unwrap_async(self) -> i32 {
        if (self.0 & !SPA_VAL_MASK) != SPA_ASYNC_BIT {
            panic!("value was not an asynchronous success");
        }
        self.0 & SPA_VAL_MASK
    }

    /// Unwrap the error code, panicking if the result was not an error.
    pub fn unwrap_error(self) -> io::Error {
        if self.0 >= 0 {
            panic!("expected error");
        }
        io::Error::from_raw_os_error(-self.0)
    }

    /// Is this result ok.
    pub fn is_ok(self) -> bool {
        self.0 & !SPA_VAL_MASK == 0
    }

    /// Is this result asynchronous.
    pub fn is_async(self) -> bool {
        self.0 & !SPA_VAL_MASK == SPA_ASYNC_BIT
    }

    /// Is this result an error.
    pub fn is_err(self) -> bool {
        self.0 < 0
    }

    /// Convert to a result, panicking if async.
    pub fn into_sync_result(self) -> io::Result<i32> {
        if self.is_async() {
            panic!("expected sync result, found async");
        }
        if self.0 >= 0 {
            Ok(self.0)
        } else {
            Err(io::Error::from_raw_os_error(-self.0))
        }
    }

    /// Convert to an async result, panicking if sync.
    pub fn into_async_result(self) -> io::Result<i32> {
        if self.is_ok() {
            panic!("expected async result, found sync");
        }
        if self.0 >= 0 {
            Ok(self.0 & SPA_VAL_MASK)
        } else {
            Err(io::Error::from_raw_os_error(-self.0))
        }
    }
}
