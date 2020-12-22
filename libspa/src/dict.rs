use bitflags::bitflags;
use std::{ffi::CStr, fmt, marker::PhantomData};

pub trait ReadableDict {
    /// Obtain the pointer to the raw `spa_dict` struct.
    fn get_dict_ptr(&self) -> *const spa_sys::spa_dict;

    /// An iterator over all raw key-value pairs.
    /// The iterator element type is `(&CStr, &CStr)`.
    fn iter_cstr(&self) -> CIter {
        let first_elem_ptr = unsafe { (*self.get_dict_ptr()).items };
        CIter {
            next: first_elem_ptr,
            end: unsafe { first_elem_ptr.offset((*self.get_dict_ptr()).n_items as isize) },
            _phantom: PhantomData,
        }
    }

    /// An iterator over all key-value pairs that are valid utf-8.
    /// The iterator element type is `(&str, &str)`.
    fn iter(&self) -> Iter {
        Iter {
            inner: self.iter_cstr(),
        }
    }

    /// An iterator over all keys that are valid utf-8.
    /// The iterator element type is &str.
    fn keys(&self) -> Keys {
        Keys {
            inner: self.iter_cstr(),
        }
    }

    /// An iterator over all values that are valid utf-8.
    /// The iterator element type is &str.
    fn values(&self) -> Values {
        Values {
            inner: self.iter_cstr(),
        }
    }

    /// Returns the number of key-value-pairs in the dict.
    /// This is the number of all pairs, not only pairs that are valid-utf8.
    fn len(&self) -> usize {
        unsafe { (*self.get_dict_ptr()).n_items as usize }
    }

    /// Returns `true` if the dict is empty, `false` if it is not.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the bitflags that are set for the dict.
    fn flags(&self) -> Flags {
        Flags::from_bits_truncate(unsafe { (*self.get_dict_ptr()).flags })
    }

    /// Get the value associated with the provided key.
    ///
    /// If the dict does not contain the key or the value is non-utf8, `None` is returned.
    /// Use [`iter_cstr`] if you need a non-utf8 key or value.
    ///
    /// [`iter_cstr`]: #method.iter_cstr
    // FIXME: Some items might be integers, booleans, floats, doubles or pointers instead of strings.
    // Perhaps we should return an enum that can be any of these values.
    // See https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/merge_requests/12#note_695914.
    fn get(&self, key: &str) -> Option<&str> {
        self.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
}

/// A wrapper for a `*const spa_dict` struct that does not take ownership of the data,
/// useful for dicts shared to us via FFI.
pub struct ForeignDict(*const spa_sys::spa_dict);

impl ForeignDict {
    /// Wraps the provided pointer in a read-only `ForeignDict` struct without taking ownership of the struct pointed to.
    ///
    /// # Safety
    ///
    /// - The provided pointer must point to a valid, well-aligned `spa_dict` struct, and must not be `NULL`.
    /// - The struct pointed to must be kept valid for the entire lifetime of the created `Dict`.
    ///
    /// Violating any of these rules will result in undefined behaviour.
    pub unsafe fn from_ptr(dict: *const spa_sys::spa_dict) -> Self {
        debug_assert!(
            !dict.is_null(),
            "Dict must not be created from a pointer that is NULL"
        );

        Self(dict)
    }
}

impl ReadableDict for ForeignDict {
    fn get_dict_ptr(&self) -> *const spa_sys::spa_dict {
        self.0
    }
}

impl fmt::Debug for ForeignDict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // FIXME: Find a way to display flags too.
        f.debug_map().entries(self.iter_cstr()).finish()
    }
}

bitflags! {
    pub struct Flags: u32 {
        // These flags are redefinitions from
        // https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/utils/dict.h
        const SORTED = 0b00000001;
    }
}

pub struct CIter<'a> {
    next: *const spa_sys::spa_dict_item,
    /// Points to the first element outside of the allocated area.
    end: *const spa_sys::spa_dict_item,
    _phantom: PhantomData<&'a str>,
}

impl<'a> Iterator for CIter<'a> {
    type Item = (&'a CStr, &'a CStr);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.next.is_null() && self.next < self.end {
            let k = unsafe { CStr::from_ptr((*self.next).key) };
            let v = unsafe { CStr::from_ptr((*self.next).value) };
            self.next = unsafe { self.next.add(1) };
            Some((k, v))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let bound: usize = unsafe { self.next.offset_from(self.end) as usize };

        // We know the exact value, so lower bound and upper bound are the same.
        (bound, Some(bound))
    }
}

pub struct Iter<'a> {
    inner: CIter<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .find_map(|(k, v)| k.to_str().ok().zip(v.to_str().ok()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Lower bound is 0, as all keys left might not be valid UTF-8.
        (0, self.inner.size_hint().1)
    }
}

pub struct Keys<'a> {
    inner: CIter<'a>,
}

impl<'a> Iterator for Keys<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.find_map(|(k, _)| k.to_str().ok())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

pub struct Values<'a> {
    inner: CIter<'a>,
}

impl<'a> Iterator for Values<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.find_map(|(_, v)| v.to_str().ok())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::{Flags, ForeignDict, ReadableDict};
    use spa_sys::{spa_dict, spa_dict_item};
    use std::{ffi::CString, ptr};

    /// Create a raw dict with the specified number of key-value pairs.
    ///
    /// `num_items` must not be zero, or this function will panic.
    ///
    /// Each key value pair is `("K<n>", "V<n>")`, with *\<n\>* being an element of the range `0..num_items`.
    ///
    /// The function returns a tuple consisting of:
    /// 1. An allocation (`Vec`) containing the raw Key and Value Strings.
    /// 2. An allocation (`Vec`) containing all the items.
    /// 3. The created `spa_dict` struct.
    ///
    /// The first two items must be kept alive for the entire lifetime of the returned `spa_dict` struct.
    fn make_raw_dict(
        num_items: u32,
    ) -> (
        Vec<(CString, CString)>,
        Vec<spa_dict_item>,
        spa_sys::spa_dict,
    ) {
        assert!(num_items != 0, "num_items must not be zero");

        let mut strings: Vec<(CString, CString)> = Vec::with_capacity(num_items as usize);
        let mut items: Vec<spa_dict_item> = Vec::with_capacity(num_items as usize);

        for i in 0..num_items {
            let k = CString::new(format!("K{}", i)).unwrap();
            let v = CString::new(format!("V{}", i)).unwrap();
            let item = spa_dict_item {
                key: k.as_ptr(),
                value: v.as_ptr(),
            };
            strings.push((k, v));
            items.push(item);
        }

        let raw = spa_dict {
            flags: Flags::empty().bits,
            n_items: num_items,
            items: items.as_ptr(),
        };

        (strings, items, raw)
    }

    #[test]
    fn test_empty_dict() {
        let raw = spa_dict {
            flags: Flags::empty().bits,
            n_items: 0,
            items: ptr::null(),
        };

        let dict = unsafe { ForeignDict::from_ptr(&raw) };
        let iter = dict.iter_cstr();

        assert_eq!(0, dict.len());

        iter.for_each(|_| panic!("Iterated over non-existing item"));
    }

    #[test]
    fn test_iter_cstr() {
        let (_strings, _items, raw) = make_raw_dict(2);
        let dict = unsafe { ForeignDict::from_ptr(&raw) };

        let mut iter = dict.iter_cstr();
        assert_eq!(
            (
                CString::new("K0").unwrap().as_c_str(),
                CString::new("V0").unwrap().as_c_str()
            ),
            iter.next().unwrap()
        );
        assert_eq!(
            (
                CString::new("K1").unwrap().as_c_str(),
                CString::new("V1").unwrap().as_c_str()
            ),
            iter.next().unwrap()
        );
        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_iterators() {
        let (_strings, _items, raw) = make_raw_dict(2);
        let dict = unsafe { ForeignDict::from_ptr(&raw) };

        let mut iter = dict.iter();
        assert_eq!(("K0", "V0"), iter.next().unwrap());
        assert_eq!(("K1", "V1"), iter.next().unwrap());
        assert_eq!(None, iter.next());

        let mut key_iter = dict.keys();
        assert_eq!("K0", key_iter.next().unwrap());
        assert_eq!("K1", key_iter.next().unwrap());
        assert_eq!(None, key_iter.next());

        let mut val_iter = dict.values();
        assert_eq!("V0", val_iter.next().unwrap());
        assert_eq!("V1", val_iter.next().unwrap());
        assert_eq!(None, val_iter.next());
    }

    #[test]
    fn test_get() {
        let (_strings, _items, raw) = make_raw_dict(1);
        let dict = unsafe { ForeignDict::from_ptr(&raw) };

        assert_eq!(Some("V0"), dict.get("K0"));
    }

    #[test]
    fn test_debug() {
        let (_strings, _items, raw) = make_raw_dict(1);
        let dict = unsafe { ForeignDict::from_ptr(&raw) };

        assert_eq!(r#"{"K0": "V0"}"#, &format!("{:?}", dict))
    }
}
