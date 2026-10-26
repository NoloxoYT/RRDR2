//! Thin wrapper around `cargo`, `dotnet` and friends with logging.

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Stdio},
};

use crate::logger::{LVL, log};

/// Runs a command from `dir` with a fully silent stdio.
///
/// stdout and stderr are discarded so the build log stays readable; the
/// command's stderr is still captured to report the real error on failure.
pub fn run_captured(dir: &Path, program: &str, args: &[String]) -> Result<(), String> {
    log(LVL::Debug, format!("{program} {}", args.join(" ")));

    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("Failed to start {program}: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let code = output
        .status
        .code()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "signal".to_string());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = stderr.lines().rev().take(20).collect();

    if tail.is_empty() {
        return Err(format!("{program} exited with status {code}"));
    }

    let mut message = format!("{program} exited with status {code}:\n");
    for line in tail.into_iter().rev() {
        message.push_str("  ");
        message.push_str(line);
        message.push('\n');
    }

    Err(message)
}

/// Runs a command and returns its stdout, trimmed.
pub fn output(dir: &Path, program: &str, args: &[String]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("Failed to start {program}: {error}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// True when a program can be found on `PATH`.
///
/// Windows only looks up `PATHEXT` extensions when the command goes through the
/// shell, so `clang-cl.exe` has to be tried explicitly.
pub fn exists(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    let mut names: Vec<String> = vec![program.to_string()];

    if cfg!(windows) && Path::new(program).extension().is_none() {
        let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".into());
        for extension in extensions.split(';').filter(|value| !value.is_empty()) {
            names.push(format!("{program}{extension}"));
        }
    }

    std::env::split_paths(&path).any(|dir| {
        names
            .iter()
            .any(|name| dir.join(OsStr::new(name)).is_file())
    })
}
