// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT
use anyhow::Error;
use libloading::{Library, Symbol};
use spa_sys::{
    spa_handle, spa_handle_factory, spa_interface_info, SPA_HANDLE_FACTORY_ENUM_FUNC_NAME,
};
use std::{
    alloc,
    collections::HashMap,
    convert::TryInto,
    ffi::{CStr, CString},
    io,
    marker::PhantomData,
    mem,
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

pub fn open(path: impl AsRef<Path>) -> Result<Plugin> {
    let path = Path::new(SPA_ROOT).join(path.as_ref());
    unsafe {
        let lib = Library::new(path)?;
        // We don't use the typedef in *sys because it wrapped in `Option` for correctness. We
        // don't need to think about a nullable function pointer because libloading handles it for
        // us.
        let enum_fn: Symbol<
            unsafe extern "C" fn(factory: *mut *const spa_handle_factory, index: *mut u32) -> c_int,
        > = lib.get(SPA_HANDLE_FACTORY_ENUM_FUNC_NAME)?;
        let mut index: u32 = 0;
        let mut factory: *const spa_handle_factory = ptr::null();
        let mut factories = HashMap::new();
        let mut factory_info = vec![];
        loop {
            let ret = err_from_ret(enum_fn(&mut factory, &mut index))?;
            if ret == 0 {
                break;
            }
            factories.insert(index, factory);
            factory_info.push(FactoryInfo::from_raw(factory, index));
        }
        Ok(Plugin {
            lib: Rc::new(lib),
            factories,
            factory_info,
        })
    }
}

#[derive(Debug)]
pub struct Plugin {
    /// A RAII handle to the shared library containing the plugin.
    // Interface handles will all share ownership of the library, so it lives at least as long as
    // the handles.
    lib: Rc<Library>,
    /// Handle factories for the plugin.
    factories: HashMap<u32, *const spa_handle_factory>,
    /// A description of the plugin.
    pub factory_info: Vec<FactoryInfo>,
}

impl Plugin {
    /// Get the index of a factory from its name.
    fn factory_from_name(&self, name: &CStr) -> Option<u32> {
        self.factory_info
            .iter()
            .filter_map(|factory| {
                if &*factory.name == name {
                    Some(factory.index)
                } else {
                    None
                }
            })
            .next()
    }

    /// Initialize a handle by name.
    pub fn init(&self, name: &[u8]) -> Option<Handle> {
        // TODO take a CStr once we can create them in `const` context.
        let name = CStr::from_bytes_with_nul(name).unwrap();
        let index = self.factory_from_name(name)?;
        unsafe {
            let factory = self.factories.get(&index).unwrap();
            let size: usize = ((**factory).get_size.unwrap())(*factory, ptr::null())
                .try_into()
                .unwrap();
            // Copy behaviour of malloc by forcing largest possible alignment.
            let mem_layout =
                alloc::Layout::from_size_align(size, align_of::<libc::max_align_t>()).unwrap();
            let handle = alloc::alloc_zeroed(mem_layout) as *mut spa_handle;
            let ret = err_from_ret(((**factory).init.unwrap())(
                *factory,
                handle,
                ptr::null(),
                ptr::null(),
                0,
            ));
            if let Err(e) = ret {
                alloc::dealloc(handle as *mut u8, mem_layout);
                panic!("init failed: {}", e);
            }
            Some(Handle {
                lib: self.lib.clone(),
                handle: Rc::new(RawHandle {
                    size,
                    inner: handle,
                }),
            })
        }
    }
}

pub struct Handle {
    lib: Rc<Library>,
    handle: Rc<RawHandle>,
}

impl Handle {
    /// Clear up after the handle.
    ///
    /// Equivalent to dropping the handle, but in addition will report errors. If there are
    /// interfaces still using the handle, this function will do nothing.
    pub fn clear(self) -> io::Result<()> {
        let Handle { lib, handle } = self;
        if let Ok(handle) = Rc::try_unwrap(handle) {
            handle.clear()
        } else {
            Ok(())
        }
    }

    /// Get an interface from the factory handle.
    ///
    /// # Safety
    ///
    /// The name must match the type of the interface.
    pub unsafe fn interface<'a, T>(&'a self, name: &[u8]) -> Result<Interface<'a, T>> {
        let name = CStr::from_bytes_with_nul(name).unwrap();
        let mut iface: *mut c_void = ptr::null_mut();
        unsafe {
            err_from_ret(((*self.handle.inner).get_interface.unwrap())(
                self.handle.inner,
                name.as_ptr(),
                &mut iface,
            ))?;
            Ok(Interface {
                handle: self.handle.clone(),
                inner: iface as *mut T,
                lifetime: PhantomData,
            })
        }
    }
}

pub struct RawHandle {
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
            err_from_ret(ret).map(|_| ())
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

pub struct Interface<'a, T> {
    handle: Rc<RawHandle>,
    inner: *mut T,
    lifetime: PhantomData<&'a ()>,
}

#[derive(Debug)]
pub struct FactoryInfo {
    /// The index this factory was at.
    pub index: u32,
    /// The version of the factory.
    pub version: u32,
    /// The readable name of the factory.
    pub name: CString,
    /// The interfaces that this factory provides.
    pub interfaces: Vec<InterfaceInfo>,
}

impl FactoryInfo {
    unsafe fn from_raw(raw: *const spa_handle_factory, index: u32) -> Self {
        let version = (*raw).version;
        let name = CStr::from_ptr((*raw).name).to_owned();
        let mut idx: u32 = 0;
        let mut interface: *const spa_interface_info = ptr::null();
        let mut interfaces = vec![];
        loop {
            let ret = err_from_ret(((*raw).enum_interface_info.unwrap())(
                raw,
                &mut interface,
                &mut idx,
            ))
            .unwrap();
            if ret == 0 {
                break;
            }
            interfaces.push(InterfaceInfo::from_raw(interface, idx));
        }
        FactoryInfo {
            index,
            version,
            name,
            interfaces,
        }
    }
}

#[derive(Debug)]
pub struct InterfaceInfo {
    pub index: u32,
    pub type_: String,
}

impl InterfaceInfo {
    unsafe fn from_raw(raw: *const spa_interface_info, index: u32) -> Self {
        let type_ = CStr::from_ptr((*raw).type_).to_string_lossy().to_string();
        InterfaceInfo { index, type_ }
    }
}

fn err_from_ret(val: i32) -> io::Result<i32> {
    if val < 0 {
        let val = -val;
        // Async test copied from macros.
        if val & (1 << 30) == 1 << 30 {
            // async in progress. io::ErrorKind doesn't have EINPROGRESS, so use `Other`.
            Err(io::Error::new(io::ErrorKind::Other, "in progress"))
        } else {
            // At time of reading, simply forwards to `strerror`, so we can go and create a
            // `io::Error`.
            Err(io::Error::from_raw_os_error(val))
        }
    } else {
        Ok(val)
    }
}
