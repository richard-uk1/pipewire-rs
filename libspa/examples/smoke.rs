use libspa::{names::SUPPORT_LOG, open, support::Log};
use log::LevelFilter;

fn main() {
    let plugin = open("support/libspa-support.so").unwrap();
    println!("{:#?}", plugin.factory_info);
    let handle = plugin.init(SUPPORT_LOG).unwrap();
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
}
