//! Install step.
//!
//! Everything can come from the command line (`--yes`, `--loader`,
//! `--game-dir`). Whatever is missing is asked interactively, and when stdin is
//! not a terminal (CI, pipes) the install is skipped with manual instructions.

use std::{
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
};

use crate::{
    cli::{Args, Loader},
    fsutil,
    logger::{LVL, log},
};

pub fn maybe_install(dist_root: &Path, args: &Args) -> Result<(), String> {
    if !args.yes && !io::stdin().is_terminal() {
        log(
            LVL::Info,
            format!(
                "Non-interactive shell: skipping install. Copy {} into the game folder.",
                dist_root.display()
            ),
        );
        return Ok(());
    }

    if !args.yes && !confirm("Install into your game folder? [y/N] ")? {
        log(LVL::Info, "Skipped install.");
        return Ok(());
    }

    let loader = match args.loader {
        Some(loader) => loader,
        None => choose_loader()?,
    };

    let game_dir = match &args.game_dir {
        Some(path) => path.clone(),
        None => choose_game_dir()?,
    };

    if !game_dir.join("RDR2.exe").is_file() {
        return Err(format!(
            "RDR2.exe not found in {}. The package is still in {}.",
            game_dir.display(),
            dist_root.display()
        ));
    }

    install(dist_root, &game_dir, loader)
}

fn confirm(prompt: &str) -> Result<bool, String> {
    let answer = ask(prompt)?;
    Ok(matches!(answer.as_str(), "y" | "Y" | "yes" | "YES"))
}

fn choose_loader() -> Result<Loader, String> {
    println!();
    println!("Which loader?");
    println!("  1) RRDR.asi   native, no .NET (recommended)");
    println!("  2) RRDR.Host.dll   managed, needs ScriptHookRDR2DotNet");

    loop {
        let answer = ask("Choice [1]: ")?;

        match answer.as_str() {
            "" | "1" | "asi" | "ASI" => return Ok(Loader::Asi),
            "2" | "dll" | "DLL" | "managed" | "dotnet" => return Ok(Loader::Host),
            _ => println!("Enter 1 or 2"),
        }
    }
}

fn choose_game_dir() -> Result<PathBuf, String> {
    let detected = detect_game_dir();

    let prompt = match &detected {
        Some(path) => format!("Game folder [{}]: ", path.display()),
        None => "Game folder (the one containing RDR2.exe): ".to_string(),
    };

    let answer = ask(&prompt)?;

    if !answer.is_empty() {
        return Ok(PathBuf::from(answer));
    }

    detected.ok_or_else(|| "No game folder given; skipping install.".to_string())
}

/// Looks in the usual places for a folder containing `RDR2.exe`.
fn detect_game_dir() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("RDR2_DIR") {
        let path = PathBuf::from(explicit);
        if path.join("RDR2.exe").is_file() {
            return Some(path);
        }
    }

    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/mnt/files/Games/Red Dead Redemption 2"),
        PathBuf::from("C:/Games/Red Dead Redemption 2"),
        PathBuf::from("D:/Games/Red Dead Redemption 2"),
    ];

    if let Some(home) = home {
        candidates.push(home.join(".wine/drive_c/Program Files/Red Dead Redemption 2"));
        candidates.push(home.join("Games/Red Dead Redemption 2"));
        candidates.push(home.join(".local/share/Steam/steamapps/common/Red Dead Redemption 2"));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.join("RDR2.exe").is_file())
}

fn install(dist_root: &Path, game_dir: &Path, loader: Loader) -> Result<(), String> {
    let cores_dir = game_dir.join("scripts/Rscripts");
    std::fs::create_dir_all(&cores_dir)
        .map_err(|error| format!("Failed to create {}: {error}", cores_dir.display()))?;

    // A host copied into scripts/ by an older package would tick the cores a
    // second time on top of RRDR.asi.
    remove_conflicting_hosts(game_dir, &cores_dir)?;

    let package_cores = dist_root.join("scripts/Rscripts");

    match loader {
        Loader::Asi => {
            fsutil::copy(&dist_root.join("RRDR.asi"), &game_dir.join("RRDR.asi"))?;
            fsutil::copy(
                &dist_root.join("RRDR.asi.ini"),
                &game_dir.join("RRDR.asi.ini"),
            )?;

            for file in fsutil::files_with_extension(&package_cores, "dll") {
                let name = fsutil::file_name(&file).unwrap_or_default();
                if name.eq_ignore_ascii_case("RRDR.Host.dll") {
                    continue;
                }
                fsutil::copy(&file, &cores_dir.join(name))?;
            }

            for file in fsutil::files_with_extension(&package_cores, "ini") {
                let name = fsutil::file_name(&file).unwrap_or_default();
                if name.eq_ignore_ascii_case("RRDR.Host.ini") {
                    continue;
                }
                fsutil::copy(&file, &cores_dir.join(name))?;
            }

            warn_if_missing(game_dir, "ScriptHookRDR2.dll");
            warn_if_missing(game_dir, "dinput8.dll");

            log(
                LVL::Success,
                format!("Installed RRDR.asi into {}", game_dir.display()),
            );
        }
        Loader::Host => {
            for file in fsutil::files_with_extension(&package_cores, "dll")
                .into_iter()
                .chain(fsutil::files_with_extension(&package_cores, "ini"))
            {
                let name = fsutil::file_name(&file).unwrap_or_default();
                fsutil::copy(&file, &cores_dir.join(name))?;
            }

            warn_if_missing(game_dir, "ScriptHookRDR2.dll");
            warn_if_missing(game_dir, "ScriptHookRDRDotNet.asi");

            log(
                LVL::Success,
                format!(
                    "Installed RRDR.Host.dll and cores into {}",
                    cores_dir.display()
                ),
            );
        }
    }

    Ok(())
}

fn remove_conflicting_hosts(game_dir: &Path, cores_dir: &Path) -> Result<(), String> {
    let mut removed = 0;

    for file in fsutil::files_recursive(game_dir, 3) {
        let name = fsutil::file_name(&file).unwrap_or_default();

        if !name.eq_ignore_ascii_case("RRDR.Host.dll")
            && !name.eq_ignore_ascii_case("RRDR.Host.ini")
        {
            continue;
        }

        if file.starts_with(cores_dir) {
            continue;
        }

        fsutil::remove(&file)?;
        log(
            LVL::Info,
            format!("Removed conflicting host: {}", file.display()),
        );
        removed += 1;
    }

    if removed == 0 {
        log(
            LVL::Info,
            "No conflicting RRDR.Host.dll outside scripts/Rscripts",
        );
    }

    Ok(())
}

fn warn_if_missing(game_dir: &Path, file: &str) {
    if !game_dir.join(file).is_file() {
        log(
            LVL::Warn,
            format!("{file} not found in {}", game_dir.display()),
        );
    }
}

/// Writes the prompt, flushes, and reads one trimmed line.
fn ask(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Failed to write to stdout: {error}"))?;

    let mut line = String::new();
    let read = io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|error| format!("Failed to read stdin: {error}"))?;

    if read == 0 {
        return Err("stdin closed; skipping install.".into());
    }

    Ok(line.trim().to_string())
}
