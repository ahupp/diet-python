use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .expect("soac-runtime should live under the repo root");
    let cpython_dir = repo_root.join("cpython");
    let header = manifest_dir.join("bindings.h");

    println!("cargo:rerun-if-changed={}", header.display());
    println!(
        "cargo:rerun-if-changed={}",
        cpython_dir.join("Include").join("object.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        cpython_dir
            .join("Include")
            .join("cpython")
            .join("object.h")
            .display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        cpython_dir
            .join("Include")
            .join("cpython")
            .join("longintrepr.h")
            .display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        cpython_dir.join("Include").join("pyport.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        cpython_dir.join("pyconfig.h").display()
    );

    let clang_args = [
        format!("-I{}", cpython_dir.display()),
        format!("-I{}", cpython_dir.join("Include").display()),
        format!("-I{}", cpython_dir.join("Include").join("cpython").display()),
        format!("-I{}", cpython_dir.join("Include").join("internal").display()),
    ];

    let bindings = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_args(clang_args)
        .raw_line("#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]")
        .allowlist_type("PyObject")
        .allowlist_type("PyVarObject")
        .allowlist_type("PyTypeObject")
        .allowlist_type("PyLongObject")
        .allowlist_type("_PyLongValue")
        .allowlist_type("digit")
        .allowlist_var("PyLong_BASE")
        .allowlist_var("PyLong_SHIFT")
        .allowlist_var("_PyLong_NON_SIZE_BITS")
        .allowlist_var("_PyLong_SIGN_MASK")
        .use_core()
        .ctypes_prefix("core::ffi")
        .size_t_is_usize(true)
        .derive_default(true)
        .layout_tests(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen failed to generate CPython bindings");

    let out_path = manifest_dir.join("src").join("cpython_bindings.rs");
    bindings
        .write_to_file(&out_path)
        .expect("bindgen failed to write bindings");

    println!("cargo:rerun-if-changed={}", out_path.display());
}
