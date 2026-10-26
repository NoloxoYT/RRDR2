//! `rrdr init <name>`: new mod crate, same idea as `cargo init`.
//!
//! Inside an RRDR checkout it creates the crate under `rust/crates/<name>/` and
//! registers it in the workspace. Anywhere else it creates a standalone crate
//! in `[dir]`, ready to be dropped into an RRDR checkout later.

use std::{
    fs,
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
};

use crate::{
    fsutil,
    logger::{LVL, log},
};

/// Location of the new crate, compared before/after `init` for the summary.
#[derive(Debug)]
pub struct Created {
    pub directory: PathBuf,
    pub library: String,
    pub in_checkout: bool,
}

/// Creates the crate named `name`.
///
/// Like `cargo init`, it writes into the current directory:
///
/// * inside an RRDR checkout: `rust/crates/<name>`;
/// * outside one: the current directory itself, if it is empty (a folder you
///   just created), or `./<name>` when the current directory has files in it.
///
/// `--path <dir>` always means "write into `<dir>` itself".
pub fn run(
    root: Option<&Path>,
    name: &str,
    dir: Option<&Path>,
    force: bool,
) -> Result<Created, String> {
    validate_name(name)?;

    let current = std::env::current_dir()
        .map_err(|error| format!("Failed to read the current directory: {error}"))?;

    let (directory, in_checkout) = match (root, dir) {
        (_, Some(dir)) => (normalise(&absolute_from(&current, dir)), false),
        (Some(root), None) => (root.join("rust/crates").join(name), true),
        (None, None) => (in_place_or_child(&current, name), false),
    };

    if is_src_dir(&directory) {
        return Err(format!(
            "{} is a `src/` directory: run the command one level up, that is where Cargo.toml goes.",
            directory.display()
        ));
    }

    create_crate(&directory, name, force, dir.is_some(), in_checkout)?;

    if in_checkout {
        let manifest = root.expect("checked").join("rust/Cargo.toml");
        add_workspace_member(&manifest, name)?;
    } else {
        log(
            LVL::Info,
            "Not inside an RRDR checkout: the crate is standalone and pulls the framework from crates.io.",
        );
    }

    Ok(Created {
        directory,
        library: library_name(name),
        in_checkout,
    })
}

/// Asks which name to use, defaulting to the directory name like `cargo init`.
///
/// `None` means "keep the default": the caller then names the package after
/// the target directory, so `rrdr init` in `hunt/` needs no typing.
pub fn ask_name() -> Result<Option<String>, String> {
    let default = std::env::current_dir()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty());

    let interactive = io::stdin().is_terminal();

    match &default {
        Some(default) if interactive => {
            print!("Mod name [{default}]: ");
        }
        Some(_) => return Ok(None),
        None if interactive => {
            print!("Mod name: ");
        }
        None => return Err("No name given. Usage: rrdr init <name>".to_string()),
    }

    io::stdout()
        .flush()
        .map_err(|error| format!("Failed to write to stdout: {error}"))?;

    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|error| format!("Failed to read stdin: {error}"))?;

    let answer = line.trim();
    if answer.is_empty() {
        return match default {
            Some(_) => Ok(None),
            None => Err("No name given. Usage: rrdr init <name>".to_string()),
        };
    }

    Ok(Some(answer.to_string()))
}

/// Same as [`run`], with the package named after the target directory.
pub fn run_default_name(
    root: Option<&Path>,
    dir: Option<&Path>,
    force: bool,
) -> Result<Created, String> {
    let directory = target_directory(root, dir)?;
    let name = crate_name_from(&directory).ok_or_else(|| {
        format!(
            "Cannot derive a crate name from {}: pass a name, e.g. `rrdr init my-mod`",
            directory.display()
        )
    })?;

    match run(root, &name, dir, force) {
        Ok(created) => Ok(created),
        Err(error) if error.contains("Invalid crate name") => {
            // `hunt the deer` or `my mod` still give a valid package name.
            let cleaned = clean_name(&name);
            log(
                LVL::Info,
                format!(
                    "Directory name `{name}` cannot be a crate name; using `{cleaned}` (rename with `name = \"{cleaned}\"` in Cargo.toml if you prefer)"
                ),
            );
            run(root, &cleaned, dir, force)
        }
        Err(error) => Err(error),
    }
}

/// Turns a directory name into something Cargo accepts.
fn clean_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if out.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '_') {
        out.insert_str(0, "mod_");
    }

    while out.contains("__") {
        out = out.replace("__", "_");
    }

    if out.is_empty() {
        "mod".to_string()
    } else {
        out
    }
}

/// Package name for a directory: `My Mod` -> `my_mod`, `2nd` -> `mod_2nd`.
fn crate_name_from(directory: &Path) -> Option<String> {
    let raw = directory.file_name()?.to_string_lossy().into_owned();
    Some(clean_name(&raw))
}

/// Where the crate will be written when no name is typed.
fn target_directory(root: Option<&Path>, dir: Option<&Path>) -> Result<PathBuf, String> {
    let current = std::env::current_dir()
        .map_err(|error| format!("Failed to read the current directory: {error}"))?;

    // Without a name, create `<cwd>/<cwd name>` only when the folder already
    // holds files; an empty folder is named after itself.
    Ok(match (root, dir) {
        (_, Some(dir)) => normalise(&absolute_from(&current, dir)),
        (Some(root), None) if is_init_target(&root.join("rust/crates")) => {
            root.join("rust/crates").join(
                current
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        }
        (Some(root), None) => root.join("rust/crates").join(
            current
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        (None, None) if is_init_target(&current) => current.clone(),
        (None, None) => current.join(
            current
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
    })
}

/// A `src/` folder is never the crate root.
fn is_src_dir(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("src")
}

/// Print the crate-created summary.
pub fn report(created: &Created) {
    log(
        LVL::Success,
        format!(
            "Created {} in {} (DLL {}.dll)",
            created.library,
            created.directory.display(),
            created.library
        ),
    );

    if created.in_checkout {
        log(
            LVL::Info,
            "Registered in rust/Cargo.toml. Run `rrdr` to build and install it.",
        );
    } else {
        log(
            LVL::Info,
            "Build it for Windows with `cargo xwin build --release --target x86_64-pc-windows-msvc`, then copy <Core>.dll and its .ini into <RDR2>/scripts/Rscripts/.",
        );
    }
}

/// Absolute form of `path`, relative to `base` when needed.
fn absolute_from(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// `cargo init` behaviour outside a workspace: write in place when the folder
/// is empty, otherwise create `<current>/<name>`.
fn in_place_or_child(current: &Path, name: &str) -> PathBuf {
    if is_init_target(current) {
        current.to_path_buf()
    } else {
        current.join(name)
    }
}

/// True when the crate can be written into `directory` in place: it does not
/// hold another crate yet, and has no source or mod files of its own
/// (`.git`, `notes.txt` and `.gitignore` are fine: that is a fresh folder).
fn is_init_target(directory: &Path) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };

    // A crate that is half created: `name` is what tells the caller to use
    // `<dir>/<name>` instead of writing here.
    if directory.join("Cargo.toml").is_file() || directory.join("src/lib.rs").exists() {
        return false;
    }

    // Cargo files that are not a crate root (`rust/Cargo.toml`, a workspace
    // manifest above `crates/`): never land a crate next to them. The workspace
    // root itself is caught too, so "run it from the checkout" never nests.
    for marker in ["Cargo.lock", "rust"] {
        if directory.join(marker).exists() {
            return false;
        }
    }
    if directory.join("Cargo.toml").is_file() && !directory.join("src").is_dir() {
        return false;
    }

    entries.filter_map(|entry| entry.ok()).all(|entry| {
        matches!(
            entry.file_name().to_str(),
            Some(".git") | Some(".gitignore")
        )
    })
}

/// Removes `.` and `..` components so the printed path is readable.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Cargo package names: alphanumerics, `-`, `_`; must start with a letter.
fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("The crate name cannot be empty".into());
    }

    if name.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '_') {
        return Err(format!(
            "Invalid crate name `{name}`: it must start with a letter"
        ));
    }

    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
    {
        return Err(format!(
            "Invalid crate name `{name}`: `{bad}` is not allowed (use letters, digits, - or _)"
        ));
    }

    if name.starts_with("rrdr") {
        return Err(format!(
            "The `rrdr-*` prefix is reserved for the framework, pick another name than `{name}`"
        ));
    }

    Ok(())
}

/// `my-mod` -> `MyModCore`, `better_economy` -> `BetterEconomyCore`.
pub fn library_name(name: &str) -> String {
    let mut pascal = String::new();
    for part in name.split(['-', '_']) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            pascal.extend(first.to_uppercase());
            pascal.extend(chars);
        }
    }

    if pascal.is_empty() {
        "ModCore".to_string()
    } else if pascal.ends_with("Core") {
        pascal
    } else {
        format!("{pascal}Core")
    }
}

fn create_crate(
    directory: &Path,
    name: &str,
    force: bool,
    strict: bool,
    in_checkout: bool,
) -> Result<(), String> {
    if strict && !force && directory.exists() && !is_init_target(directory) {
        let has_manifest = directory.join("Cargo.toml").is_file();
        return Err(format!(
            "{} already exists and is not empty{}. Use --force to write the template anyway.",
            directory.display(),
            if has_manifest {
                " (it already has a Cargo.toml: is this mod already created?)"
            } else {
                ""
            }
        ));
    }

    let library = library_name(name);
    let package_slug = library.to_lowercase();

    write(
        &directory.join("Cargo.toml"),
        &cargo_toml(name, &library, in_checkout),
        force,
    )?;
    write(&directory.join("src/lib.rs"), &lib_rs(), force)?;
    write(
        &directory.join(format!("{library}.ini")),
        &ini(&package_slug),
        force,
    )?;
    write(&directory.join(".gitignore"), "/target\n", force)?;

    Ok(())
}

fn write(path: &Path, contents: &str, force: bool) -> Result<(), String> {
    if path.exists() && !force {
        return Err(format!("{} already exists", path.display()));
    }

    fsutil::copy_bytes(path, contents.as_bytes())
}

fn cargo_toml(name: &str, library: &str, in_checkout: bool) -> String {
    // Inside the RRDR workspace the version/edition/license come from
    // `[workspace.package]`; standalone the crate carries its own and pulls the
    // framework from crates.io, so it builds on its own.
    let meta = if in_checkout {
        "version.workspace = true\nedition.workspace = true\nlicense.workspace = true"
    } else {
        "version = \"0.1.0\"\nedition = \"2021\"\nlicense = \"MIT\""
    };

    let bridge = if in_checkout {
        r#"rrdr-bridge = { path = "../rrdr-bridge" }
rrdr-game = { path = "../rrdr-game" }
rrdr-natives = { path = "../rrdr-natives" }"#
    } else {
        r#"rrdr-bridge = "0.1"
rrdr-game = "0.1"
rrdr-natives = "0.1""#
    };

    format!(
        r#"[package]
name = "{name}"
{meta}
description = "{library} core for the RDR2 Rust mod framework"

[lib]
name = "{library}"
crate-type = ["cdylib", "rlib"]
path = "src/lib.rs"

[dependencies]
{bridge}
"#
    )
}

fn lib_rs() -> String {
    r#"#![allow(non_snake_case)]

use rrdr_bridge::{export_mod, GameApi, LogLevel, Mod};
use rrdr_game::prelude::*;
use std::path::Path;

// `Mod::init` returns `std::result::Result<_, String>`, while the prelude's
// `Result` is the game error type.

struct MyMod {
    elapsed: f32,
}

impl Mod for MyMod {
    fn init(api: &mut dyn GameApi, _config_path: Option<&Path>) -> std::result::Result<Self, String> {
        api.log(LogLevel::Info, "mod loaded");
        Ok(Self { elapsed: 0.0 })
    }

    fn tick(&mut self, api: &mut dyn GameApi, dt: f32) {
        self.elapsed += dt;
        if self.elapsed < 1.0 {
            return;
        }
        self.elapsed = 0.0;

        let Some(player) = Player::local(api) else {
            return;
        };
        let Some(ped) = player.ped_checked(api) else {
            return;
        };
        let health = ped.health(api);

        api.log(LogLevel::Debug, &format!("player health: {health}"));
    }
}

export_mod!(MyMod);
"#
    .to_string()
}

fn ini(slug: &str) -> String {
    format!(
        r#"# {slug} configuration.
# The loader resolves this file from the DLL name: <Core>.dll -> <Core>.ini
# It is reloaded when its modification time changes.

# Master switch.
enabled = true

[debug]
# Log verbose steps to the loader log.
debug = false
"#
    )
}

/// Adds the crate to `rust/Cargo.toml` members, keeping the list sorted.
fn add_workspace_member(manifest: &Path, name: &str) -> Result<(), String> {
    let mut text = fs::read_to_string(manifest)
        .map_err(|error| format!("Failed to read {}: {error}", manifest.display()))?;

    if text.contains(&format!("\"crates/{name}\"")) {
        return Ok(());
    }

    let Some(members_start) = text.find("members = [") else {
        return Err(format!(
            "{} has no `members = [` list; add \"crates/{name}\" by hand",
            manifest.display()
        ));
    };
    let list_start = members_start + "members = [".len();
    let Some(relative_end) = text[list_start..].find(']') else {
        return Err(format!(
            "{} has an unterminated members list",
            manifest.display()
        ));
    };
    let list_end = list_start + relative_end;

    let mut members: Vec<String> = text[list_start..list_end]
        .lines()
        .map(|line| line.trim().trim_end_matches(',').to_string())
        .filter(|line| !line.is_empty())
        .collect();
    members.push(format!("\"crates/{name}\""));
    members.sort();

    let mut block = String::new();
    for member in members {
        block.push_str("    ");
        block.push_str(&member);
        block.push_str(",\n");
    }

    let replacement = format!("\n{block}");
    text.replace_range(list_start..list_end, &replacement);

    fs::write(manifest, text)
        .map_err(|error| format!("Failed to write {}: {error}", manifest.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_name_is_pascal_case() {
        assert_eq!(library_name("my-mod"), "MyModCore");
        assert_eq!(library_name("better_economy"), "BetterEconomyCore");
        assert_eq!(library_name("hunt"), "HuntCore");
    }

    #[test]
    fn names_are_validated() {
        assert!(validate_name("my-mod").is_ok());
        assert!(validate_name("good_mod_2").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("2mod").is_err());
        assert!(validate_name("bad name").is_err());
        assert!(validate_name("rrdr-thing").is_err());
    }
}
