// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use pipewire as pw;
use signal::Signal;
use std::{cell::RefCell, collections::HashMap};
use std::{rc::Rc, sync::Arc};
use structopt::StructOpt;

use pw::link::Link;
use pw::node::Node;
use pw::port::Port;
use pw::prelude::*;
use pw::properties;
use pw::proxy::{Listener, ProxyListener, ProxyT};
use pw::registry::ObjectType;

struct Proxies {
    proxies_t: HashMap<u32, Box<dyn ProxyT>>,
    listeners: HashMap<u32, Vec<Box<dyn Listener>>>,
}

impl Proxies {
    fn new() -> Self {
        Self {
            proxies_t: HashMap::new(),
            listeners: HashMap::new(),
        }
    }

    fn add_proxy_t(&mut self, proxy_t: Box<dyn ProxyT>, listener: Box<dyn Listener>) {
        let proxy_id = {
            let proxy = proxy_t.upcast_ref();
            proxy.id()
        };

        self.proxies_t.insert(proxy_id, proxy_t);

        let v = self.listeners.entry(proxy_id).or_insert_with(Vec::new);
        v.push(listener);
    }

    fn add_proxy_listener(&mut self, proxy_id: u32, listener: ProxyListener) {
        let v = self.listeners.entry(proxy_id).or_insert_with(Vec::new);
        v.push(Box::new(listener));
    }

    fn remove(&mut self, proxy_id: u32) {
        self.proxies_t.remove(&proxy_id);
        self.listeners.remove(&proxy_id);
    }
}

fn monitor(remote: Option<String>) -> Result<()> {
    let main_loop = pw::MainLoop::new()?;

    let main_loop_weak = main_loop.downgrade();
    let _sig_int = main_loop.add_signal_local(Signal::SIGINT, move || {
        if let Some(main_loop) = main_loop_weak.upgrade() {
            main_loop.quit();
        }
    });
    let main_loop_weak = main_loop.downgrade();
    let _sig_term = main_loop.add_signal_local(Signal::SIGTERM, move || {
        if let Some(main_loop) = main_loop_weak.upgrade() {
            main_loop.quit();
        }
    });

    let context = pw::Context::new(&main_loop)?;
    let props = remote.map(|remote| {
        properties! {
            // TODO: define constants from keys.h
            "remote.name" => remote
        }
    });
    let core = context.connect(props)?;

    let main_loop_weak = main_loop.downgrade();
    let _listener = core
        .add_listener_local()
        .info(|info| {
            dbg!(info);
        })
        .done(|_id, _seq| {
            // TODO
        })
        .error(move |id, seq, res, message| {
            eprintln!("error id:{} seq:{} res:{}: {}", id, seq, res, message);

            if id == 0 {
                if let Some(main_loop) = main_loop_weak.upgrade() {
                    main_loop.quit();
                }
            }
        })
        .register();

    let registry = Arc::new(core.get_registry());
    let registry_weak = Arc::downgrade(&registry);

    // Proxies and their listeners need to stay alive so store them here
    let proxies = Rc::new(RefCell::new(Proxies::new()));

    let _registry_listener = registry
        .add_listener_local()
        .global(move |obj| {
            if let Some(registry) = registry_weak.upgrade() {
                let p: Option<(Box<dyn ProxyT>, Box<dyn Listener>)> = match obj.type_ {
                    ObjectType::Node => {
                        let node: Node = registry.bind(&obj).unwrap();
                        let obj_listener = node
                            .add_listener_local()
                            .info(|info| {
                                dbg!(info);
                            })
                            .param(|seq, id, index, next| {
                                dbg!((seq, id, index, next));
                            })
                            .register();

                        Some((Box::new(node), Box::new(obj_listener)))
                    }
                    ObjectType::Port => {
                        let port: Port = registry.bind(&obj).unwrap();
                        let obj_listener = port
                            .add_listener_local()
                            .info(|info| {
                                dbg!(info);
                            })
                            .param(|seq, id, index, next| {
                                dbg!((seq, id, index, next));
                            })
                            .register();

                        Some((Box::new(port), Box::new(obj_listener)))
                    }
                    ObjectType::Link => {
                        let link: Link = registry.bind(&obj).unwrap();
                        let obj_listener = link
                            .add_listener_local()
                            .info(|info| {
                                dbg!(info);
                            })
                            .register();

                        Some((Box::new(link), Box::new(obj_listener)))
                    }
                    ObjectType::Module
                    | ObjectType::Device
                    | ObjectType::Factory
                    | ObjectType::Client => {
                        // TODO
                        None
                    }
                    _ => {
                        dbg!(obj);
                        None
                    }
                };

                if let Some((proxy_spe, listener_spe)) = p {
                    let proxy = proxy_spe.upcast_ref();
                    let proxy_id = proxy.id();
                    // Use a weak ref to prevent references cycle between Proxy and proxies:
                    // - ref on proxies in the closure, bound to the Proxy lifetime
                    // - proxies owning a ref on Proxy as well
                    let proxies_weak = Rc::downgrade(&proxies);

                    let listener = proxy
                        .add_listener_local()
                        .removed(move || {
                            if let Some(proxies) = proxies_weak.upgrade() {
                                proxies.borrow_mut().remove(proxy_id);
                            }
                        })
                        .register();

                    proxies.borrow_mut().add_proxy_t(proxy_spe, listener_spe);
                    proxies.borrow_mut().add_proxy_listener(proxy_id, listener);
                }
            }
        })
        .global_remove(|id| {
            println!("removed:");
            println!("\tid: {}", id);
        })
        .register();

    main_loop.run();

    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "pw-mon", about = "PipeWire monitor")]
struct Opt {
    #[structopt(short, long, help = "The name of the remote to connect to")]
    remote: Option<String>,
}

fn main() -> Result<()> {
    pw::init();

    let opt = Opt::from_args();
    monitor(opt.remote)?;

    unsafe {
        pw::deinit();
    }

    Ok(())
}
