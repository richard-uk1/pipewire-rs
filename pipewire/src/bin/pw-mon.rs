// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use pipewire as pw;
use signal::Signal;
use std::sync::Arc;

fn monitor() -> Result<()> {
    let main_loop = Arc::new(pw::MainLoop::new()?);
    let l = main_loop.get_loop();

    let main_loop_weak = Arc::downgrade(&main_loop);
    let _sig_int = l.add_signal_local(Signal::SIGINT, move || {
        if let Some(main_loop) = main_loop_weak.upgrade() {
            main_loop.quit();
        }
    });
    let main_loop_weak = Arc::downgrade(&main_loop);
    let _sig_term = l.add_signal_local(Signal::SIGTERM, move || {
        if let Some(main_loop) = main_loop_weak.upgrade() {
            main_loop.quit();
        }
    });

    let context = pw::Context::new(&l)?;
    // TODO: pass properties to connect
    let _core = context.connect()?;

    // TODO: _core.add_listener()
    // TODO: get registry and add listener

    main_loop.run();

    Ok(())
}

fn main() -> Result<()> {
    pw::init();

    // TODO: add arguments

    monitor()?;

    unsafe {
        pw::deinit();
    }

    Ok(())
}
