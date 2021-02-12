use libspa::{
    names::{SUPPORT_CPU, SUPPORT_LOG},
    support::{Cpu, Log},
    Plugin,
};
use log::LevelFilter;

fn main() {
    let plugin = Plugin::open("support/libspa-support.so").unwrap();
    println!("{:#?}", plugin);
    let handle = plugin.factory(SUPPORT_LOG).unwrap().instantiate();
    let mut logger = Log::from_handle(&handle).unwrap();
    println!("{:?}", logger.level());
    libspa::error!(logger, "an error");
    libspa::warn!(logger, "a warning");
    libspa::info!(logger, "info");
    libspa::debug!(logger, "debug");
    libspa::trace!(logger, "a trace");
    logger.set_level(LevelFilter::Trace);
    libspa::debug!(logger, "debug");
    libspa::trace!(logger, "a trace");
    println!();

    let handle = plugin.factory(SUPPORT_CPU).unwrap().instantiate();
    let cpu = Cpu::from_handle(&handle).unwrap();
    libspa::info!(logger, "Cpu flags: {:b}", cpu.flags());
    libspa::info!(logger, "Cpu count: {}", cpu.count());
    libspa::info!(logger, "Cpu max align: {}", cpu.max_align());
    println!();
}
