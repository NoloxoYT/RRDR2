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
/// `dir` is `--path`: the parent directory to create the crate in. Without it
/// the crate goes to `<root>/rust/crates/<name>` when `root` is an RRDR
/// checkout, and errors otherwise (a bare `--no-checkout`-style call is not
/// useful for mods, which need the framework crates).
pub fn run(
    root: Option<&Path>,
    name: &str,
    dir: Option<&Path>,
    force: bool,
) -> Result<Created, String> {
    validate_name(name)?;

    let in_checkout = root.is_some() && dir.is_none();

    let directory = match (in_checkout, dir) {
        (true, _) => root.expect("checked").join("rust/crates").join(name),
        (false, Some(parent)) => {
            let parent = if parent.is_absolute() {
                parent.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(|error| format!("Failed to read the current directory: {error}"))?
                    .join(parent)
            };
            normalise(&parent.join(name))
        }
        (false, None) => {
            return Err(
                "`rrdr init` needs an RRDR checkout to create a mod in, or --path <dir>. \
                 Run it from a checkout, or pass --root <dir> / --path <dir>."
                    .to_string(),
            );
        }
    };

    create_crate(&directory, name, force)?;

    if in_checkout {
        let manifest = root.expect("checked").join("rust/Cargo.toml");
        add_workspace_member(&manifest, name)?;
    } else {
        log(
            LVL::Info,
            "Standalone crate: I did not find rust/Cargo.toml next to it, so it is not registered in an RRDR workspace.",
        );
    }

    Ok(Created {
        directory,
        library: library_name(name),
        in_checkout,
    })
}

/// Asks which name to use, defaulting to the directory name like `cargo init`.
pub fn ask_name() -> Result<String, String> {
    let default = std::env::current_dir()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_default();

    if !io::stdin().is_terminal() {
        return default.is_empty().then_some(default).ok_or_else(|| {
            "No name given (and stdin is not a terminal). Usage: rrdr init <name>".to_string()
        });
    }

    let prompt = if default.is_empty() {
        "Mod name: ".to_string()
    } else {
        format!("Mod name [{default}]: ")
    };

    print!("{prompt}");
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
        if default.is_empty() {
            return Err("No name given. Usage: rrdr init <name>".to_string());
        }
        return Ok(default);
    }

    Ok(answer.to_string())
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
            "Move it under rust/crates/ of a checkout and add it to the workspace members, or run rrdr init again from the checkout.",
        );
    }
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

fn create_crate(directory: &Path, name: &str, force: bool) -> Result<(), String> {
    if directory.exists() {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| format!("Failed to read {}: {error}", directory.display()))?;
        if !force && entries.next().is_some() {
            return Err(format!(
                "{} already exists and is not empty (use --force to write the template anyway)",
                directory.display()
            ));
        }
    }

    let library = library_name(name);
    let package_slug = library.to_lowercase();

    write(
        &directory.join("Cargo.toml"),
        &cargo_toml(name, &library),
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

fn cargo_toml(name: &str, library: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "{library} core for the RDR2 Rust bridge"

[lib]
name = "{library}"
crate-type = ["cdylib", "rlib"]
path = "src/lib.rs"

[dependencies]
rrdr-bridge = {{ path = "../rrdr-bridge" }}
rrdr-game = {{ path = "../rrdr-game" }}
rrdr-natives = {{ path = "../rrdr-natives" }}
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
