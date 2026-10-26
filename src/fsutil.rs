//! Filesystem helpers shared by the build steps.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Copies a file, creating parent directories as needed.
pub fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    fs::copy(from, to).map_err(|error| {
        format!(
            "Failed to copy {} to {}: {error}",
            from.display(),
            to.display()
        )
    })?;

    Ok(())
}

/// Removes a path if it exists; missing paths are not an error.
pub fn remove(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Failed to remove {}: {error}", path.display()))
    } else {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to remove {}: {error}", path.display()))
    }
}

/// Lists regular files in a directory with the given extension (no leading dot).
pub fn files_with_extension(directory: &Path, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case(extension))
                .unwrap_or(false)
        })
        .collect();

    files.sort();
    files
}

/// Lists files in a directory, recursively, capped at `depth` levels.
pub fn files_recursive(directory: &Path, depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(directory, depth, &mut found);
    found
}

fn walk(directory: &Path, remaining: usize, found: &mut Vec<PathBuf>) {
    if remaining == 0 {
        return;
    }

    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();

        if path.is_dir() {
            walk(&path, remaining - 1, found);
        } else {
            found.push(path);
        }
    }
}

/// File name with extension, as a `String`.
pub fn file_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
}
