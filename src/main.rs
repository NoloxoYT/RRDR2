//! `rrdr`: build and install RRDR, and create mods.
//!
//! `rrdr` (no subcommand) runs from a checkout of the repository (`--root <dir>`).
//! It runs the tests, the simulator, the Windows cores, the managed host and the
//! native ASI loader, then offers to install the result.
//!
//! `rrdr init <name>` creates a new mod crate, like `cargo init`.

mod asi;
mod build;
mod cli;
mod dist;
mod fsutil;
mod init;
mod logger;
mod menu;
mod process;

use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::Parser;

use crate::{
    cli::{Cli, Command},
    logger::{LVL, log},
};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init {
            name,
            path,
            force,
            root,
        }) => run_init(root.as_deref(), name.as_deref(), path.as_deref(), force),
        None => run_build(&cli),
    }
}

fn run_init(root: Option<&Path>, name: Option<&str>, path: Option<&Path>, force: bool) -> ExitCode {
    // A checkout is optional for `init`; outside one, `--path` drives the location.
    let root = match root {
        Some(root) => match resolve_root(Some(root)) {
            Ok(root) => Some(root),
            Err(error) => {
                log(LVL::Error, error);
                return ExitCode::FAILURE;
            }
        },
        None => resolve_root(None).ok(),
    };

    let name = match name {
        Some(name) => Some(name.to_string()),
        None => match init::ask_name() {
            Ok(name) => name,
            Err(error) => {
                log(LVL::Error, error);
                return ExitCode::FAILURE;
            }
        },
    };

    let Some(name) = name else {
        // No prompt answer: name the package after the target directory.
        return match init::run_default_name(root.as_deref(), path, force) {
            Ok(created) => {
                init::report(&created);
                ExitCode::SUCCESS
            }
            Err(error) => {
                log(LVL::Error, error);
                ExitCode::FAILURE
            }
        };
    };

    match init::run(root.as_deref(), &name, path, force) {
        Ok(created) => {
            init::report(&created);
            ExitCode::SUCCESS
        }
        Err(error) => {
            log(LVL::Error, error);
            ExitCode::FAILURE
        }
    }
}

fn run_build(cli: &Cli) -> ExitCode {
    let root = match resolve_root(cli.build.root.as_deref()) {
        Ok(root) => root,
        Err(error) => {
            log(LVL::Error, error);
            return ExitCode::FAILURE;
        }
    };

    match build::run(&root, &cli.build) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log(LVL::Error, error);
            ExitCode::FAILURE
        }
    }
}

/// Finds the RRDR checkout: `--root`, then `$RRDR_ROOT`, then the current
/// directory and its parents.
///
/// The binary's own location is deliberately not used: once `rrdr` is installed
/// in `~/.cargo/bin`, that location says nothing about the repository.
fn resolve_root(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return validate_root(path).ok_or_else(|| {
            format!(
                "--root {} is not an RRDR checkout (no rust/Cargo.toml there)",
                path.display()
            )
        });
    }

    if let Some(path) = env::var_os("RRDR_ROOT") {
        let path = PathBuf::from(path);
        return validate_root(&path).ok_or_else(|| {
            format!(
                "RRDR_ROOT ({}) is not an RRDR checkout (no rust/Cargo.toml there)",
                path.display()
            )
        });
    }

    let current = env::current_dir()
        .map_err(|error| format!("Failed to read the current directory: {error}"))?;

    walk_up(&current).ok_or_else(|| {
        "Could not find an RRDR checkout: no rust/Cargo.toml in the current directory \
         or any parent. Run rrdr from a checkout, or pass --root <dir>."
            .to_string()
    })
}

/// Returns the first directory at or above `start` that looks like a checkout.
fn walk_up(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|candidate| {
        candidate
            .join("rust/Cargo.toml")
            .is_file()
            .then(|| candidate.to_path_buf())
    })
}

fn validate_root(path: &Path) -> Option<PathBuf> {
    let absolute = std::fs::canonicalize(path).ok()?;
    absolute
        .join("rust/Cargo.toml")
        .is_file()
        .then_some(absolute)
}

#[cfg(test)]
fn test_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_repository_root_from_nested_directory() {
        let root = test_root().canonicalize().expect("root");
        let nested = root.join("build_tools/src");

        assert_eq!(walk_up(&nested), Some(root.clone()));
    }

    #[test]
    fn walk_up_returns_none_outside_a_checkout() {
        let outside = std::env::temp_dir().join("rrdr-no-checkout-here");
        std::fs::create_dir_all(&outside).expect("temp dir");

        assert_eq!(walk_up(&outside), None);
    }

    #[test]
    fn explicit_root_wins_over_environment() {
        let root = test_root().canonicalize().expect("root");
        assert_eq!(resolve_root(Some(&root)).expect("root"), root);
    }

    #[test]
    fn invalid_explicit_root_is_an_error() {
        let outside = std::env::temp_dir().join("rrdr-not-a-checkout");

        assert!(resolve_root(Some(&outside)).is_err());
    }
}
