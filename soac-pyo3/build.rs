fn main() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace crate should have a repo-root parent");
    let python_lib_dir = repo_root.join("vendor/cpython");
    let python_link_name =
        find_python_shared_lib_name(&python_lib_dir).expect("expected vendored shared libpython");
    let python_lib_dir = python_lib_dir.display();
    println!("cargo:rustc-link-search=native={python_lib_dir}");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{python_lib_dir}");
    println!("cargo:rustc-link-lib=dylib={python_link_name}");
}

fn find_python_shared_lib_name(dir: &std::path::Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries {
        let path = entry.ok()?.path();
        let file_name = path.file_name()?.to_str()?;
        if !file_name.starts_with("libpython") || !file_name.ends_with(".so") {
            continue;
        }
        return file_name
            .strip_prefix("lib")
            .and_then(|name| name.strip_suffix(".so"))
            .map(ToOwned::to_owned);
    }
    None
}
