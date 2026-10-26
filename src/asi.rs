//! Builds `RRDR.asi` from `native/RRDR.Asi/RRDR.Asi.cpp`.
//!
//! On Linux this reuses the Windows SDK headers that `cargo-xwin` downloaded,
//! so no Visual Studio install is required. On Windows it relies on the
//! toolchain being on `PATH` (clang-cl and lld-link).

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::logger::{LVL, log};

const SOURCE: &str = "native/RRDR.Asi/RRDR.Asi.cpp";

pub fn build(root: &Path, output: &Path) -> Result<(), String> {
    let source = root.join(SOURCE);
    if !source.is_file() {
        return Err(format!("Missing {}", source.display()));
    }

    if cfg!(windows) {
        build_for_windows(root, &source, output)
    } else {
        build_for_linux(root, &source, output)
    }
}

fn build_for_linux(root: &Path, source: &Path, output: &Path) -> Result<(), String> {
    let xwin = xwin_dir().ok_or_else(|| {
        "Windows headers not found. Run a `cargo xwin build` once to download them, \
         or set XWIN_DIR."
            .to_string()
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let object = std::env::temp_dir().join("RRDR.Asi.obj");

    let mut compile = Command::new("clang-cl");
    compile
        .current_dir(root)
        .arg("--target=x86_64-pc-windows-msvc")
        .arg("-O2")
        .arg("-fms-extensions")
        .arg(format!("-imsvc{}", xwin.join("crt/include").display()))
        .arg(format!("-imsvc{}", xwin.join("sdk/include/ucrt").display()))
        .arg(format!(
            "-imsvc{}",
            xwin.join("sdk/include/shared").display()
        ))
        .arg(format!("-imsvc{}", xwin.join("sdk/include/um").display()))
        .arg("/c")
        .arg(source)
        .arg(format!("/Fo{}", object.display()));

    run(&mut compile, "clang-cl")?;

    let mut link = Command::new("lld-link");
    link.arg("/dll")
        .arg("/noentry")
        .arg("/machine:x64")
        .arg("/nodefaultlib")
        .arg(format!("/out:{}", output.display()))
        .arg(xwin.join("crt/lib/x86_64/libcmt.lib"))
        .arg(xwin.join("crt/lib/x86_64/libcpmt.lib"))
        .arg(xwin.join("crt/lib/x86_64/libvcruntime.lib"))
        .arg(xwin.join("sdk/lib/ucrt/x86_64/ucrt.lib"))
        .arg(xwin.join("sdk/lib/um/x86_64/kernel32.lib"))
        .arg(xwin.join("sdk/lib/um/x86_64/user32.lib"))
        .arg(&object);

    run(&mut link, "lld-link")?;

    let _ = std::fs::remove_file(&object);
    log(LVL::Success, format!("Wrote {}", output.display()));

    Ok(())
}

fn build_for_windows(root: &Path, source: &Path, output: &Path) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let object = std::env::temp_dir().join("RRDR.Asi.obj");

    let mut compile = Command::new("clang-cl");
    compile
        .current_dir(root)
        .arg("/c")
        .arg(source)
        .arg(format!("/Fo{}", object.display()))
        .arg("/O2")
        .arg("/EHsc");

    run(&mut compile, "clang-cl")?;

    let mut link = Command::new("lld-link");
    link.arg("/dll")
        .arg("/noentry")
        .arg("/machine:x64")
        .arg(format!("/out:{}", output.display()))
        .arg(&object)
        .arg("kernel32.lib")
        .arg("user32.lib");

    run(&mut link, "lld-link")?;

    let _ = std::fs::remove_file(&object);
    log(LVL::Success, format!("Wrote {}", output.display()));

    Ok(())
}

fn run(command: &mut Command, program: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("Failed to start {program}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed with status {status}"))
    }
}

fn xwin_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(explicit) = std::env::var("XWIN_DIR") {
        candidates.push(PathBuf::from(explicit));
    }

    // cargo-xwin honours these too, so a non-default location still works.
    for variable in ["XWIN_CACHE_DIR", "XDG_CACHE_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            let path = PathBuf::from(value);
            candidates.push(path.join("cargo-xwin/xwin"));
            candidates.push(path.join("xwin"));
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join(".cache/cargo-xwin/xwin"));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.join("sdk/include/um").is_dir())
}
