//! Types and methods to wrap the "support" standard plugin.

use log::{Level, LevelFilter};
use spa_sys::{spa_log, spa_log_methods};
use std::{convert::TryInto, ffi::CString, os::raw::c_uint};

use crate::{names::SUPPORT_LOG, Handle, Interface, Result};

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

    pub fn log(&self, level: Level, file: &'static str, line: u32, msg: impl Into<String>) {
        #[cold]
        if level <= self.level() {
            let mut msg = msg.into().into_bytes();
            msg.push(b'\n');
            let msg = CString::new(msg).unwrap();
            let level = match level {
                Level::Error => 1,
                Level::Warn => 2,
                Level::Info => 3,
                Level::Debug => 4,
                Level::Trace => 5,
            };
            unsafe {
                crate::spa_interface_call_method!(
                    self.interface.inner,
                    spa_log_methods,
                    log,
                    level,
                    file as *const str as *mut _,
                    line.try_into().unwrap(),
                    b"-" as *const [u8; 1] as *mut _,
                    msg.as_ptr()
                )
            }
        }
    }
}

#[macro_export]
macro_rules! error {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        $logger.log(log::Level::Error, file!(), line!(), msg)
    }
}

#[macro_export]
macro_rules! warn {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        $logger.log(log::Level::Warn, file!(), line!(), msg)
    }
}

#[macro_export]
macro_rules! info {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        $logger.log(log::Level::Info, file!(), line!(), msg)
    }
}

#[macro_export]
macro_rules! debug {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        $logger.log(log::Level::Debug, file!(), line!(), msg)
    }
}

#[macro_export]
macro_rules! trace {
    ($logger:expr, $fmt:expr$(, $fmt_args:expr)*) => {
        let msg = format!($fmt, $($fmt_args),*);
        $logger.log(log::Level::Trace, file!(), line!(), msg)
    }
}
