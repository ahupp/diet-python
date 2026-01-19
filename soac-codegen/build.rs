use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUNTIME_CRATE: &str = "soac-runtime";
const RUNTIME_CRATE_UNDERSCORE: &str = "soac_runtime";

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir
        .parent()
        .ok_or("soac-codegen should live under the repo root")?;
    let runtime_dir = repo_root.join("soac-runtime");
    let target_dir = soac_codegen_target_dir(repo_root);

    emit_rerun_if_changed(&runtime_dir)?;

    build_runtime(repo_root, &target_dir)?;

    let clif_files = find_runtime_clif_files(&target_dir)?;
    if clif_files.is_empty() {
        return Err(
            "no .clif files found for soac-runtime; ensure the build emits CLIF output"
                .to_string()
                .into(),
        );
    }

    let clif_text = read_clif_files(&clif_files)?;
    write_clif_constant(&clif_text)?;

    Ok(())
}

fn emit_rerun_if_changed(runtime_dir: &Path) -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={}", runtime_dir.display());
    for path in walk_files(runtime_dir)? {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    Ok(())
}

fn build_runtime(repo_root: &Path, target_dir: &Path) -> Result<(), Box<dyn Error>> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);

    command
        .current_dir(repo_root.join("soac-runtime"))
        .env("CARGO_PROFILE_DEV_CODEGEN_BACKEND", "cranelift")
        .env("CARGO_TARGET_DIR", target_dir)
        .arg("rustc")
        .arg("-vv")
        .arg("-Zcodegen-backend")
        .arg("-p")
        .arg(RUNTIME_CRATE)
        .arg("--lib")
        .arg("--")
        .arg("--emit=llvm-ir")
        .arg("-Ccodegen-units=1");
 
    let status = command.status()?;
    if !status.success() {
        return Err("failed to build soac-runtime; run `rustup component add rustc-codegen-cranelift-preview --toolchain nightly`"
            .to_string()
            .into());
    }
    Ok(())
}


fn soac_codegen_target_dir(repo_root: &Path) -> PathBuf {
    let target_dir = repo_root.join("target").join("soac-codegen-clif");
    let _ = fs::create_dir_all(&target_dir);
    target_dir
}

fn find_runtime_clif_files(target_dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut clif_files = Vec::new();
    collect_clif_files(target_dir, &mut clif_files)?;

    clif_files.retain(|path| is_runtime_clif_path(path));
    clif_files.sort();
    clif_files.dedup();

    Ok(clif_files)
}

fn collect_clif_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_clif_files(&path, out)?;
        } else if path.extension() == Some(OsStr::new("clif")) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_runtime_clif_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.contains(RUNTIME_CRATE) || name.contains(RUNTIME_CRATE_UNDERSCORE)
    })
}

fn read_clif_files(paths: &[PathBuf]) -> Result<String, Box<dyn Error>> {
    let mut out = String::new();
    for (idx, path) in paths.iter().enumerate() {
        if idx > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&fs::read_to_string(path)?);
    }
    Ok(out)
}

fn write_clif_constant(clif_text: &str) -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let out_path = out_dir.join("soac_clif.rs");
    let literal = raw_string_literal(clif_text);
    let contents = format!("pub const SOAC_CLIF: &str = {};\n", literal);
    fs::write(out_path, contents)?;
    Ok(())
}

fn raw_string_literal(text: &str) -> String {
    let mut max_run = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '#' {
            current += 1;
            if current > max_run {
                max_run = current;
            }
        } else {
            current = 0;
        }
    }
    let hashes = "#".repeat(max_run + 1);
    format!("r{hashes}\"{text}\"{hashes}")
}

fn walk_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    walk_files_inner(dir, &mut files)?;
    Ok(files)
}

fn walk_files_inner(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            walk_files_inner(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
