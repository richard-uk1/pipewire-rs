// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::env;
use std::path::PathBuf;

fn main() {
    let libs = system_deps::Config::new()
        .probe()
        .expect("Cannot find libspa");
    let libspa = libs.get("libspa").unwrap();

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    let builder = bindgen::Builder::default()
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .whitelist_function("spa_.*")
        .whitelist_type("spa_.*")
        .whitelist_var("spa_.*")
        .whitelist_var("SPA_.*");

    let builder = libspa.include_paths.iter().fold(builder, |builder, l| {
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
