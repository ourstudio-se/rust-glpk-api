use std::env;
use std::path::PathBuf;

fn main() {
    // Get Hexaly installation path from environment variable
    let hexaly_home = env::var("HEXALY_HOME")
        .or_else(|_| env::var("LOCALSOLVER_HOME"))
        .expect("HEXALY_HOME or LOCALSOLVER_HOME environment variable must be set");

    let hexaly_path = PathBuf::from(&hexaly_home);

    // Determine include and lib paths based on platform
    // Hexaly 14.5 uses libhexaly145.dylib (on macOS) or libhexaly145.so (on Linux)
    let (include_path, lib_path, lib_name) = if cfg!(target_os = "macos") {
        (
            hexaly_path.join("include"),
            hexaly_path.join("bin"),
            "hexaly145",
        )
    } else if cfg!(target_os = "linux") {
        (
            hexaly_path.join("include"),
            hexaly_path.join("bin"),
            "hexaly145",
        )
    } else if cfg!(target_os = "windows") {
        (
            hexaly_path.join("include"),
            hexaly_path.join("bin"),
            "hexaly145", // Version may vary
        )
    } else {
        panic!("Unsupported platform");
    };

    // Verify paths exist
    if !include_path.exists() {
        panic!(
            "Hexaly include directory not found at: {}",
            include_path.display()
        );
    }
    if !lib_path.exists() {
        panic!(
            "Hexaly library directory not found at: {}",
            lib_path.display()
        );
    }

    println!("cargo:rerun-if-changed=hexaly_wrapper.h");
    println!("cargo:rerun-if-changed=hexaly_wrapper.cpp");
    println!("cargo:rerun-if-env-changed=HEXALY_HOME");
    println!("cargo:rerun-if-env-changed=LOCALSOLVER_HOME");

    // Compile the C++ wrapper
    cc::Build::new()
        .cpp(true)
        .file("hexaly_wrapper.cpp")
        .include(&include_path)
        .flag_if_supported("-std=c++11")
        .flag_if_supported("/std:c++11")
        .warnings(false) // Suppress warnings from wrapper
        .compile("hexaly_wrapper");

    // Link against Hexaly library
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=dylib={}", lib_name);

    // On macOS, we may need to set the rpath
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }

    // Generate Rust bindings
    let bindings = bindgen::Builder::default()
        .header("hexaly_wrapper.h")
        .clang_arg(format!("-I{}", include_path.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("hxw_.*")
        .allowlist_type("Hx.*Wrapper")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
