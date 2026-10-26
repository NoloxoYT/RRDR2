//! Mod discovery and the build pipeline.

use std::{fs, path::Path};

use crate::{
    asi,
    cli::Args,
    dist, fsutil,
    logger::{LVL, Mtype, log},
    menu, process,
};

/// A mod crate: its Cargo package name and the DLL it produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModCrate {
    pub package: String,
    pub library: String,
}

const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";
const WINDOWS_PROFILE: &str = "release";

impl ModCrate {
    /// `RealisticDamagesCore.dll`
    pub fn dll_name(&self) -> String {
        format!("{}.dll", self.library)
    }
}

fn header(title: &str) {
    log(
        LVL::Other {
            mtype: Mtype::Header,
        },
        title,
    );
}

fn content(message: impl std::fmt::Display) {
    log(
        LVL::Other {
            mtype: Mtype::Content,
        },
        message,
    );
}

/// Runs every step, then offers to install.
pub fn run(root: &Path, args: &Args) -> Result<(), String> {
    let mods = discover_mods(root)?;

    header("Mod crates");
    for entry in &mods {
        content(format!("{} -> {}", entry.package, entry.dll_name()));
    }

    let manifest = root.join("rust/Cargo.toml");
    let manifest_arg = path_arg(&manifest);
    let native_windows = cfg!(windows);

    preflight(root, native_windows)?;

    if !args.skip_tests {
        header("Rust tests");
        process::run_captured(
            root,
            "cargo",
            &[
                "test".into(),
                "--manifest-path".into(),
                manifest_arg.clone(),
                "--workspace".into(),
            ],
        )?;
    }

    header("Rust debug build");
    process::run_captured(
        root,
        "cargo",
        &[
            "build".into(),
            "--manifest-path".into(),
            manifest_arg.clone(),
        ],
    )?;

    if !args.skip_sim {
        header("End-to-end simulator");
        let simulator = root.join("lib/managed/RRDR.Sim/RRDR.Sim.csproj");
        process::run_captured(
            root,
            "dotnet",
            &[
                "run".into(),
                "--project".into(),
                path_arg(&simulator),
                "-c".into(),
                "Debug".into(),
            ],
        )?;
    }

    let windows_dir = build_windows_cores(root, &manifest_arg, &mods, native_windows)?;

    header("Managed host");
    let host_project = root.join("lib/managed/RRDR.Host/RRDR.Host.csproj");
    process::run_captured(
        root,
        "dotnet",
        &[
            "build".into(),
            path_arg(&host_project),
            "-c".into(),
            "Release".into(),
            "-f".into(),
            "net48".into(),
            "-p:UseRealApi=true".into(),
        ],
    )?;

    // dist/ is rebuilt from scratch every time.
    let dist_root = root.join("dist/RRDR");
    fsutil::remove(&root.join("dist"))?;

    header("Package");
    dist::assemble(root, &mods, &dist_root, &windows_dir)?;

    header("Native ASI loader");
    asi::build(root, &dist_root.join("RRDR.asi"))?;

    header("Package ready");
    dist::print(&dist_root);

    if args.no_install {
        log(
            LVL::Info,
            format!(
                "--no-install: package left in {}. Copy it into the game folder when ready.",
                dist_root.display()
            ),
        );
        return Ok(());
    }

    menu::maybe_install(&dist_root, args)
}

/// Cross-compiles every core for Windows and returns the folder holding them.
///
/// On a Windows host the default target is already MSVC, so no `--target` and
/// no `cargo-xwin` are needed.
fn build_windows_cores(
    root: &Path,
    manifest: &str,
    mods: &[ModCrate],
    native_windows: bool,
) -> Result<std::path::PathBuf, String> {
    header("Windows cores");

    let mut args = windows_core_args(manifest, native_windows);

    for entry in mods {
        args.push("-p".into());
        args.push(entry.package.clone());
    }

    process::run_captured(root, "cargo", &args)?;

    let windows_dir = if native_windows {
        root.join(format!("rust/target/{WINDOWS_PROFILE}"))
    } else {
        root.join(format!("rust/target/{WINDOWS_TARGET}/{WINDOWS_PROFILE}"))
    };

    for entry in mods {
        let dll = windows_dir.join(entry.dll_name());
        if !dll.is_file() {
            return Err(format!("Missing {} after the Windows build", dll.display()));
        }
    }

    Ok(windows_dir)
}

/// `cargo [-xwin] build ...` arguments for the Windows cores.
fn windows_core_args(manifest: &str, native_windows: bool) -> Vec<String> {
    let mut args: Vec<String> = vec!["build".into()];

    if !native_windows {
        // `cargo xwin build` is a cargo subcommand.
        args = vec!["xwin".into(), "build".into()];
        args.push("--target".into());
        args.push(WINDOWS_TARGET.into());
    }

    args.push("--manifest-path".into());
    args.push(manifest.into());
    args.push("--release".into());
    args
}

/// Checks the tools the pipeline needs, so a missing one fails before the
/// long steps rather than halfway through.
fn preflight(root: &Path, native_windows: bool) -> Result<(), String> {
    let mut missing: Vec<String> = Vec::new();

    if !process::exists("cargo") {
        missing.push("cargo (install Rust from https://rustup.rs)".into());
    }

    if !process::exists("dotnet") {
        missing.push("dotnet (.NET SDK, https://dotnet.microsoft.com/download)".into());
    }

    if !native_windows && !process::exists("cargo-xwin") {
        missing.push("cargo-xwin (cargo install cargo-xwin --locked)".into());
    }

    if !process::exists("clang-cl") || !process::exists("lld-link") {
        missing.push("clang-cl and lld-link (LLVM, or Visual Studio Build Tools)".into());
    }

    if !missing.is_empty() {
        let mut message = String::from("Missing build tools:\n");
        for entry in missing {
            message.push_str("  - ");
            message.push_str(&entry);
            message.push('\n');
        }
        return Err(message);
    }

    if !native_windows {
        ensure_windows_target(root)?;
    }

    Ok(())
}

/// Confirms the MSVC target is installed through rustup.
fn ensure_windows_target(root: &Path) -> Result<(), String> {
    let installed = process::output(
        root,
        "rustup",
        &["target".into(), "list".into(), "--installed".into()],
    )?;

    if !installed.lines().any(|line| line.trim() == WINDOWS_TARGET) {
        return Err(format!(
            "The {WINDOWS_TARGET} target is missing. Add it with: rustup target add {WINDOWS_TARGET}"
        ));
    }

    Ok(())
}

/// Reads `rust/crates/*/Cargo.toml` and keeps the `cdylib` crates.
pub fn discover_mods(root: &Path) -> Result<Vec<ModCrate>, String> {
    let crates_dir = root.join("rust/crates");
    let entries = fs::read_dir(&crates_dir)
        .map_err(|error| format!("Failed to read {}: {error}", crates_dir.display()))?;

    let mut mods = Vec::new();

    for entry in entries.filter_map(|entry| entry.ok()) {
        let manifest_path = entry.path().join("Cargo.toml");
        let Ok(manifest) = fs::read_to_string(&manifest_path) else {
            continue;
        };

        if !manifest.contains("cdylib") {
            continue;
        }

        let Some(package) = toml_string(&manifest) else {
            continue;
        };

        let section = manifest.split("[lib]").nth(1).unwrap_or("");
        let library = toml_string(section).unwrap_or_else(|| package.replace('-', "_"));

        mods.push(ModCrate { package, library });
    }

    mods.sort_by(|left, right| left.package.cmp(&right.package));

    if mods.is_empty() {
        return Err("No cdylib crate found under rust/crates/".into());
    }

    Ok(mods)
}

/// Extracts the first `name = "..."` from a manifest fragment.
fn toml_string(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("name") {
            continue;
        }

        let (key, value) = line.split_once('=')?;
        if key.trim() != "name" {
            continue;
        }

        let value = value.trim();
        let value = value.strip_prefix('"')?.split('"').next()?;
        return Some(value.to_string());
    }

    None
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_cdylib_crates_only() {
        let root = crate::test_root();
        let mods = discover_mods(&root).expect("mods");

        let packages: Vec<&str> = mods.iter().map(|entry| entry.package.as_str()).collect();
        assert!(packages.contains(&"better_economy"));
        assert!(packages.contains(&"realistic-damages"));
        assert!(!packages.contains(&"rrdr-bridge"));
        assert!(!packages.contains(&"rrdr-natives"));
        assert!(!packages.contains(&"rrdr-game"));
    }

    #[test]
    fn library_name_differs_from_package_name() {
        let root = crate::test_root();
        let mods = discover_mods(&root).expect("mods");

        let realistic = mods
            .iter()
            .find(|entry| entry.package == "realistic-damages")
            .expect("realistic-damages");

        assert_eq!(realistic.library, "RealisticDamagesCore");
        assert_eq!(realistic.dll_name(), "RealisticDamagesCore.dll");
    }

    #[test]
    fn dash_becomes_underscore_without_lib_section() {
        let text = "[package]\nname = \"my-mod\"\n\n[dependencies]\n";
        let package = toml_string(text).expect("package");
        assert_eq!(package, "my-mod");
    }

    #[test]
    fn windows_args_use_xwin_off_windows() {
        let args = windows_core_args("rust/Cargo.toml", false);

        assert_eq!(&args[..2], ["xwin", "build"]);
        assert!(args.iter().any(|arg| arg == WINDOWS_TARGET));
        assert!(args.contains(&"--release".to_string()));
    }

    #[test]
    fn windows_args_use_native_target_on_windows() {
        let args = windows_core_args("rust/Cargo.toml", true);

        assert_eq!(args[0], "build");
        assert!(!args.contains(&"xwin".to_string()));
        assert!(!args.contains(&WINDOWS_TARGET.to_string()));
    }

    #[test]
    fn toml_string_reads_first_name() {
        let text = "[package]\nname = \"alpha\"\n\n[lib]\nname = \"Beta\"\n";
        assert_eq!(toml_string(text).as_deref(), Some("alpha"));
        assert_eq!(
            toml_string(text.split("[lib]").nth(1).unwrap()).as_deref(),
            Some("Beta")
        );
    }
}
