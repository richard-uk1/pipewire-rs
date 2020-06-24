use std::env;
use std::path::PathBuf;

fn main() {
    let libs = system_deps::Config::new()
        .probe()
        .expect("Cannot find libpipewire");
    let libpipewire = libs.get("libpipewire").unwrap();

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    let builder = bindgen::Builder::default()
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .whitelist_function("pw_.*")
        .whitelist_type("pw_.*")
        .whitelist_var("pw_.*");

    let builder = libpipewire
        .include_paths
        .iter()
        .fold(builder, |builder, l| {
            let arg = format!("-I{}", l.to_string_lossy());
            builder.clang_arg(arg)
        });

    let bindings = builder.generate().expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}