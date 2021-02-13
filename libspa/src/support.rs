//! Types and methods to wrap the "support" standard plugin.

use log::{Level, LevelFilter};
use spa_sys::{
    spa_cpu, spa_cpu_methods, spa_log, spa_log_methods, spa_loop, spa_system, spa_system_methods,
};
use std::{
    convert::TryInto,
    ffi::CString,
    io,
    os::raw::{c_int, c_void},
};

use crate::{interface::Interface, SpaResult};

// Log

pub struct Log<'a> {
    raw: &'a mut spa_log,
}

unsafe impl<'a> Interface<'a> for Log<'a> {
    const NAME: &'static [u8] = b"Spa:Pointer:Interface:Log\0";
    // TODO spa_interface_info reports version 1, but the actual interface report version 0. I
    // don't know why this is.
    const VERSION: u32 = 0;
    type Type = spa_log;

    fn from_raw(raw: &'a mut spa_log) -> Self {
        Log { raw }
    }
}

impl<'a> Log<'a> {
    /// The lowest level of log messages that will be displayed.
    pub fn level(&self) -> LevelFilter {
        match self.raw.level {
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
        self.raw.level = level;
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
    pub unsafe fn log(
        &mut self,
        level: Level,
        file: &'static str,
        line: u32,
        msg: impl Into<String>,
    ) {
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
                self.raw as *mut spa_log,
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
    raw: &'a mut spa_system,
}

unsafe impl<'a> Interface<'a> for System<'a> {
    const NAME: &'static [u8] = b"Spa:Pointer:Interface:System\0";
    const VERSION: u32 = 0;
    type Type = spa_system;

    fn from_raw(raw: &'a mut spa_system) -> Self {
        System { raw }
    }
}

impl<'a> System<'a> {
    /// Access to the `read` syscall
    ///
    /// # Safety
    ///
    /// Matches safety requirements of the underlying syscall.
    pub unsafe fn read(&mut self, fd: c_int, buf: *mut c_void, count: u64) -> i64 {
        crate::spa_interface_call_method!(
            self.raw as *mut spa_system,
            spa_system_methods,
            read,
            fd,
            buf,
            count
        )
    }
}

// CPU

pub struct Cpu<'a> {
    raw: &'a mut spa_cpu,
}

unsafe impl<'a> Interface<'a> for Cpu<'a> {
    const NAME: &'static [u8] = b"Spa:Pointer:Interface:CPU\0";
    const VERSION: u32 = 0;
    type Type = spa_cpu;

    fn from_raw(raw: &'a mut spa_cpu) -> Self {
        Cpu { raw }
    }
}

impl<'a> Cpu<'a> {
    pub fn flags(&mut self) -> u32 {
        unsafe {
            crate::spa_interface_call_method!(self.raw as *mut spa_cpu, spa_cpu_methods, get_flags,)
        }
    }

    pub fn force_flags(&mut self, flags: u32) -> io::Result<()> {
        SpaResult::from_raw(unsafe {
            crate::spa_interface_call_method!(
                self.raw as *mut spa_cpu,
                spa_cpu_methods,
                force_flags,
                flags
            )
        })
        .into_sync_result()
        .map(|_| ())
    }

    pub fn count(&mut self) -> u32 {
        unsafe {
            crate::spa_interface_call_method!(self.raw as *mut spa_cpu, spa_cpu_methods, get_count,)
        }
    }

    pub fn max_align(&mut self) -> u32 {
        unsafe {
            crate::spa_interface_call_method!(
                self.raw as *mut spa_cpu,
                spa_cpu_methods,
                get_max_align,
            )
        }
    }
}

// Loop

pub struct Loop<'a> {
    raw: &'a mut spa_loop,
}

unsafe impl<'a> Interface<'a> for Loop<'a> {
    const NAME: &'static [u8] = b"Spa:Pointer:Interface:Loop\0";
    const VERSION: u32 = 0;
    type Type = spa_loop;

    fn from_raw(raw: &'a mut spa_loop) -> Self {
        Loop { raw }
    }
}

/*
impl<'a> Loop<'a> {
    pub fn add_soiurce
}
*/
