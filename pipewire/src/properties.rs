use spa::dict::{ReadableDict, WritableDict};
use std::{ffi::CString, mem::ManuallyDrop};

/// A collection of key/value pairs.
///
/// # Examples
/// Create a `Properties` struct and access the stored values by key:
/// ```rust
/// use pipewire::{properties, Properties};
/// use spa::dict::ReadableDict;
///
/// let props = properties!{
///     "Key" => "Value",
///     "OtherKey" => "OtherValue"
/// };
///
/// assert_eq!(Some("Value"), props.get("Key"));
/// assert_eq!(Some("OtherValue"), props.get("OtherKey"));
/// ```
pub struct Properties {
    ptr: *mut pw_sys::pw_properties,
}

/// A macro for creating a new `Properties` struct with predefined key-value pairs.
///
/// The macro accepts a list of `Key => Value` pairs, seperated by commas.
///
/// # Examples:
/// Create a `Properties` struct from literals.
/// ```rust
/// use pipewire::properties;
///
/// let props = properties!{
///    "Key1" => "Value1",
///    "Key2" => "Value2"
/// };
/// ```
///
/// Any expression that evaluates to a `impl Into<Vec<u8>>` can be used for both keys and values.
/// ```rust
/// use pipewire::properties;
/// use spa::dict::ReadableDict;
///
/// let key = String::from("Key");
/// let value = vec![86, 97, 108, 117, 101]; // "Value" as an ASCII u8 vector.
/// let props = properties!{
///     key => value
/// };
///
/// assert_eq!(Some("Value"), props.get("Key"));
/// ```
#[macro_export]
macro_rules! properties {
    {$($k:expr => $v:expr),+} => {{
        unsafe {
            $crate::Properties::from_ptr(pw_sys::pw_properties_new(
                $(
                    std::ffi::CString::new($k).unwrap().as_ptr(),
                    std::ffi::CString::new($v).unwrap().as_ptr()
                ),+,
                std::ptr::null::<std::os::raw::c_char>()
            )
        )
        }
    }};
}

impl Properties {
    /// Create a `Properties` struct from an existing raw `pw_properties` pointer.
    ///
    /// # Safety
    /// - The provided pointer must point to a valid, well-aligned `pw_properties` struct, and must not be `NULL`.
    /// - After this call, the generated `Properties` struct will assume ownership of the data pointed to,
    ///   so that data must not be freed elsewhere.
    pub unsafe fn from_ptr(ptr: *mut pw_sys::pw_properties) -> Self {
        debug_assert!(!ptr.is_null());

        Self { ptr }
    }

    /// Obtain a pointer to the underlying `pw_properties` struct.
    ///
    /// The pointer is only valid for the lifetime of the `Properties` struct the pointer was obtained from,
    /// and must not be dereferenced after it is dropped.
    ///
    /// Ownership of the `pw_properties` struct is not transferred to the caller and must not be manually freed.
    /// If you want to take ownership, use [into_raw()](Self::into_raw()) instead.
    pub fn as_ptr(&self) -> *mut pw_sys::pw_properties {
        self.ptr
    }

    /// Consume the `Properties` struct, returning a pointer to the raw `pw_properties` struct.
    ///
    /// After this function, the caller is responsible for `pw_properties` struct,
    /// and should make sure it is freed when it is no longer needed.
    pub fn into_raw(self) -> *mut pw_sys::pw_properties {
        let this = ManuallyDrop::new(self);

        this.ptr
    }

    // TODO: `fn from_string` that calls `pw_sys::pw_properties_new_string`
    // TODO: bindings for pw_properties_update_keys, pw_properties_update, pw_properties_add, pw_properties_add_keys
}

impl ReadableDict for Properties {
    fn get_dict_ptr(&self) -> *const spa_sys::spa_dict {
        self.ptr as *const spa_sys::spa_dict
    }
}

impl WritableDict for Properties {
    fn insert<T: Into<Vec<u8>>>(&mut self, key: T, value: T) {
        let k = CString::new(key).unwrap();
        let v = CString::new(value).unwrap();
        unsafe { pw_sys::pw_properties_set(self.ptr, k.as_ptr(), v.as_ptr()) };
    }

    fn remove<T: Into<Vec<u8>>>(&mut self, key: T) {
        let key = CString::new(key).unwrap();
        unsafe { pw_sys::pw_properties_set(self.ptr, key.as_ptr(), std::ptr::null()) };
    }

    fn clear(&mut self) {
        unsafe { pw_sys::pw_properties_clear(self.ptr) }
    }
}

impl Clone for Properties {
    fn clone(&self) -> Self {
        unsafe {
            let ptr = pw_sys::pw_properties_copy(self.ptr);

            Self { ptr }
        }
    }
}

impl Drop for Properties {
    fn drop(&mut self) {
        unsafe { pw_sys::pw_properties_free(self.ptr) }
    }
}

#[cfg(test)]
mod tests {
    use spa::dict::{ReadableDict, WritableDict};

    #[test]
    fn new() {
        let props = properties! {
            "K0" => "V0"
        };

        let mut iter = props.iter();
        assert_eq!(("K0", "V0"), iter.next().unwrap());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn remove() {
        let mut props = properties! {
            "K0" => "V0"
        };

        assert_eq!(Some("V0"), props.get("K0"));
        props.remove("K0");
        assert_eq!(None, props.get("K0"));
    }

    #[test]
    fn insert() {
        let mut props = properties! {
            "K0" => "V0"
        };

        assert_eq!(None, props.get("K1"));
        props.insert("K1", "V1");
        assert_eq!(Some("V1"), props.get("K1"));
    }

    #[test]
    fn clone() {
        let props1 = properties! {
            "K0" => "V0"
        };
        let mut props2 = props1.clone();

        props2.insert("K1", "V1");

        // Now, props2 should contain ("K1", "V1"), but props1 should not.

        assert_eq!(None, props1.get("K1"));
        assert_eq!(Some("V1"), props2.get("K1"));
    }
}
