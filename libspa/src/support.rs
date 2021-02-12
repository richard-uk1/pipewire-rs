//! Types and methods to wrap the "support" standard plugin.

use log::{Level, LevelFilter};
use spa_sys::{spa_cpu, spa_cpu_methods, spa_log, spa_log_methods, spa_system, spa_system_methods};
use std::{convert::TryInto, ffi::CString, io};

use crate::{names::SUPPORT_LOG, Handle, Interface, Result};

// Log

pub struct Log<'a> {
    interface: Interface<'a, spa_log>,
}

impl<'a> Log<'a> {
    pub fn from_handle(handle: &'a Handle) -> Result<Self> {
        // TODO I think the guide is out of date here.
        let interface = unsafe { handle.interface(b"Spa:Pointer:Interface:Log\0")? };
        Ok(Log { interface })
    }

    /// The lowest level of log messages that will be displayed.
    pub fn level(&self) -> LevelFilter {
        match unsafe { (*self.interface.inner).level } {
            0 => LevelFilter::Off,
            1 => LevelFilter::Error,
            2 => LevelFilter::Warn,
            3 => LevelFilter::Info,
            4 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }

    pub fn set_level(&mut self, level: LevelFilter) {
        let level = match level {
            LevelFilter::Off => 0,
            LevelFilter::Error => 1,
            LevelFilter::Warn => 2,
            LevelFilter::Info => 3,
            LevelFilter::Debug => 4,
            LevelFilter::Trace => 5,
        };
        unsafe {
            (*self.interface.inner).level = level;
        }
    }

    /// Log a message.
    ///
    /// # Safety
    ///
    /// The `file` string must be null-terminated.
    ///
    /// # Panics
    ///
    /// The function will panic if `msg` contains interior null bytes.
    pub unsafe fn log(&self, level: Level, file: &'static str, line: u32, msg: impl Into<String>) {
        // TODO mark branch unlikely once rustc supports this (currently only possible using
        // nightly-only intrinsics)
        if level <= self.level() {
            let mut msg = msg.into().into_bytes();
            msg.push(b'\n'); // match crate `log` behavior.
            let msg = CString::new(msg).unwrap();
            let level = match level {
                Level::Error => 1,
                Level::Warn => 2,
                Level::Info => 3,
                Level::Debug => 4,
                Level::Trace => 5,
            };
            crate::spa_interface_call_method!(
                self.interface.inner,
                spa_log_methods,
                log,
                level,
                file as *const str as *mut _,
                line.try_into().unwrap(),
                b"-\0" as *const [u8; 2] as *mut _,
                msg.as_ptr()
            )
        }
    }
}

/// Log an error
#[macro_export]
macro_rules! error {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        unsafe { $logger.log(log::Level::Error, concat!(file!(), "\0"), line!(), msg) }
    }
}

/// Log a warning
#[macro_export]
macro_rules! warn {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        unsafe { $logger.log(log::Level::Warn, concat!(file!(), "\0"), line!(), msg) }
    }
}

/// Log some information
#[macro_export]
macro_rules! info {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        unsafe { $logger.log(log::Level::Info, concat!(file!(), "\0"), line!(), msg) }
    }
}

/// Log some debug information
#[macro_export]
macro_rules! debug {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        unsafe { $logger.log(log::Level::Debug, concat!(file!(), "\0"), line!(), msg) }
    }
}

/// Log some detailed debug information.
#[macro_export]
macro_rules! trace {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        unsafe { $logger.log(log::Level::Trace, concat!(file!(), "\0"), line!(), msg) }
    }
}

// System

/// Access to syscalls.
///
/// Currently a stub. TODO add methods
pub struct System<'a> {
    interface: Interface<'a, spa_system>,
}

impl<'a> System<'a> {
    pub fn from_handle(handle: &'a Handle) -> Result<Self> {
        let interface = unsafe { handle.interface(b"Spa:Pointer:Interface:System\0")? };
        Ok(System { interface })
    }
}

// CPU

pub struct Cpu<'a> {
    interface: Interface<'a, spa_cpu>,
}

impl<'a> Cpu<'a> {
    pub fn from_handle(handle: &'a Handle) -> Result<Self> {
        let interface = unsafe { handle.interface(b"Spa:Pointer:Interface:CPU\0")? };
        Ok(Cpu { interface })
    }

    pub fn flags(&self) -> u32 {
        unsafe {
            crate::spa_interface_call_method!(self.interface.inner, spa_cpu_methods, get_flags,)
        }
    }

    pub fn force_flags(&mut self, flags: u32) -> io::Result<()> {
        crate::err_from_code(unsafe {
            crate::spa_interface_call_method!(
                self.interface.inner,
                spa_cpu_methods,
                force_flags,
                flags
            )
        })
        .map(|_| ())
    }

    pub fn count(&self) -> u32 {
        unsafe {
            crate::spa_interface_call_method!(self.interface.inner, spa_cpu_methods, get_count,)
        }
    }

    pub fn max_align(&self) -> u32 {
        unsafe {
            crate::spa_interface_call_method!(self.interface.inner, spa_cpu_methods, get_max_align,)
        }
    }
}
