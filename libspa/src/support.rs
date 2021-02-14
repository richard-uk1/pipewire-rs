//! Types and methods to wrap the "support" standard plugin.
//!
//! TODO make it so you can set the global logger to a `Log`.

use log::{Level, LevelFilter};
use spa_sys::{
    spa_cpu, spa_cpu_methods, spa_log, spa_log_methods, spa_loop, spa_system, spa_system_methods,
};
use std::{
    convert::TryInto,
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
    /// Don't use this function directly: instead use the macros. This function always prints the
    /// message, regarless of the level filter.
    ///
    /// # Safety
    ///
    /// The `file` string must be null-terminated.
    ///
    /// # Panics
    ///
    /// The function will panic if `msg` contains interior null bytes.
    // TODO currently allocates. I can't see how to bridge between `println` and `printf`
    // semantics for formatting text withouta allocating.
    #[doc(hidden)]
    pub unsafe fn _log(&mut self, level: Level, file: &'static str, line: u32, msg: String) {
        let mut msg = msg.into_bytes();
        // CString would panic on interior null bytes, we just pass this string rhrough to display
        // up to the null byte. Also add a newline to match `log` crate behavior`.
        msg.push(b'\n');
        msg.push(b'\0');
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
            file as *const str as *mut _, // Safety: user responsibility to be null-terminated.
            line.try_into().unwrap(),
            b"-\0" as *const [u8; 2] as *mut _, // unsupported.
            msg.as_ptr() as *const _
        )
    }
}

/// Log a message
/// Use the other macros (`error`, `warn`, `info`, `debug`, `trace`) to avoid having to specify a level.
#[macro_export]
macro_rules! log {
    ($logger:expr, $level:expr, $($fmt_args:tt)+) => {
        // TODO mark branch unlikely once rustc supports this (currently only possible using
        // nightly-only intrinsics)
        if $level <= $logger.level() {
            let msg = format!($($fmt_args)+);
            unsafe { $logger._log($level, concat!(file!(), "\0"), line!(), msg) }
        }
    }
}

/// Log an error
#[macro_export]
macro_rules! error {
    ($logger:expr, $($fmt_args:tt)+) => {
        $crate::log!($logger, log::Level::Error, $($fmt_args)+)
    }
}

/// Log a warning
#[macro_export]
macro_rules! warn {
    ($logger:expr, $($fmt_args:tt)+) => {
        $crate::log!($logger, log::Level::Warn, $($fmt_args)+)
    }
}

/// Log some information
#[macro_export]
macro_rules! info {
    ($logger:expr, $($fmt_args:tt)+) => {
        $crate::log!($logger, log::Level::Info, $($fmt_args)+)
    }
}

/// Log some debug information
#[macro_export]
macro_rules! debug {
    ($logger:expr, $($fmt_args:tt)+) => {
        $crate::log!($logger, log::Level::Debug, $($fmt_args)+)
    }
}

/// Log some detailed debug information.
#[macro_export]
macro_rules! trace {
    ($logger:expr, $($fmt_args:tt)+) => {
        $crate::log!($logger, log::Level::Trace, $($fmt_args)+)
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
