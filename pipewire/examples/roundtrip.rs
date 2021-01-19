//! This program is the rust equivalent of https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/doc/tutorial3.md.

use pipewire::*;
use std::{cell::Cell, rc::Rc};

fn main() {
    pipewire::init();

    roundtrip();

    unsafe { pipewire::deinit() };
}

fn roundtrip() {
    let mainloop = MainLoop::new().expect("Failed to create main loop");
    let context = Context::new(&mainloop).expect("Failed to create context");
    let core = context.connect(None).expect("Failed to connect to core");
    let registry = core.get_registry();

    // To comply with Rust's safety rules, we wrap this variable in an `Rc` and  a `Cell`.
    let done = Rc::new(Cell::new(false));

    // Create new reference for each variable so that they can be moved into the closure.
    let done_clone = done.clone();
    let loop_clone = mainloop.clone();

    // Trigger the sync event. The server's answer won't be processed until we start the main loop,
    // so we can safely do this before setting up a callback. This lets us avoid using a Cell.
    let pending = core.sync(0);

    let _listener_core = core
        .add_listener_local()
        .done(move |id, seq| {
            if id == PW_ID_CORE && seq == pending {
                done_clone.set(true);
                loop_clone.quit();
            }
        })
        .register();
    let _listener_reg = registry
        .add_listener_local()
        .global(|global| {
            println!(
                "object: id:{} type:{}/{}",
                global.id, global.type_, global.version
            )
        })
        .register();

    while !done.get() {
        mainloop.run();
    }
}
