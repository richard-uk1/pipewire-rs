use std::rc::Rc;

use once_cell::unsync::OnceCell;
use pipewire as pw;
use pw::types::ObjectType;
use spa::dict::ReadableDict;

fn main() {
    // Initialize library and get the basic structures we need.
    pw::init();
    let mainloop = pw::MainLoop::new().expect("Failed to create Pipewire Mainloop");
    let context = pw::Context::new(&mainloop).expect("Failed to create Pipewire Context");
    let core = context
        .connect(None)
        .expect("Failed to connect to Pipewire Core");
    let registry = core.get_registry();

    // Setup a registry listener that will obtain the name of a link factory and write it into `factory`.
    let factory: Rc<OnceCell<String>> = Rc::new(OnceCell::new());
    let factory_clone = factory.clone();
    let mainloop_clone = mainloop.clone();
    let reg_listener = registry
        .add_listener_local()
        .global(move |global| {
            if let Some(ref props) = global.props {
                // Check that the global is a factory that creates the right type.
                if props.get("factory.type.name") == Some(ObjectType::Link.to_str()) {
                    let factory_name = props.get("factory.name").expect("Factory has no name");
                    factory_clone
                        .set(factory_name.to_owned())
                        .expect("Factory name already set");
                    // We found the factory we needed, so quit the loop.
                    mainloop_clone.quit();
                }
            }
        })
        .register();

    // Run the main loop to get the factory.
    // If no link factory is found, the loop won't terminate, but that's fine for this example.
    mainloop.run();

    // Now that we have our factory, we are no longer interested in any globals from the registry,
    // so we unregister the listener by dropping it.
    std::mem::drop(reg_listener);

    // Now that we have the name of a link factory, we can create an object with it!
    let _link = core
        .create_object::<pw::link::Link, _>(
            factory.get().expect("No link factory found"),
            &pw::properties! {
                "link.output.port" => "1",
                "link.input.port" => "2",
                "link.output.node" => "3",
                "link.input.node" => "4"
                /* TODO: Uncomment this once the object is manually deleted from the remote
                // Don't remove the object on the remote when we destroy our proxy.
                "object.linger" => "1"
                */
            },
        )
        .expect("Failed to create object");

    // TODO: Manually destroy the object on the remote again.
}
