//! Assembles `dist/RRDR/`.
//!
//! Layout:
//!
//! ```text
//! dist/RRDR/
//!   RRDR.asi
//!   RRDR.asi.ini
//!   RRDR.lib
//!   README.txt
//!   scripts/Rscripts/
//!     <core>.dll, <Core>.ini
//!     RRDR.Host.dll, RRDR.Host.ini
//! ```

use std::path::{Path, PathBuf};

use crate::{build::ModCrate, fsutil};

pub fn assemble(
    root: &Path,
    mods: &[ModCrate],
    dist_root: &Path,
    windows_dir: &Path,
) -> Result<(), String> {
    let cores_dir = dist_root.join("scripts/Rscripts");
    std::fs::create_dir_all(&cores_dir)
        .map_err(|error| format!("Failed to create {}: {error}", cores_dir.display()))?;

    let host_dir = root.join("lib/managed/RRDR.Host/bin/Release/net48");

    for file in ["RRDR.Host.dll", "RRDR.Host.ini"] {
        fsutil::copy(&host_dir.join(file), &cores_dir.join(file))?;
    }

    let default_config = root.join("lib/managed/RRDR.Host/RealisticDamages.ini");
    if default_config.is_file() {
        fsutil::copy(&default_config, &cores_dir.join("RealisticDamages.ini"))?;
    }

    for entry in mods {
        let dll = windows_dir.join(entry.dll_name());
        fsutil::copy(&dll, &cores_dir.join(entry.dll_name()))?;

        // Ship a matching config when the crate provides one.
        let ini = root.join(format!(
            "rust/crates/{}/{}.ini",
            entry.package, entry.library
        ));
        if ini.is_file() {
            let destination = cores_dir.join(format!("{}.ini", entry.library));
            if !destination.exists() {
                fsutil::copy(&ini, &destination)?;
            }
        }
    }

    // Ship the ASI config next to the loader; without it the ASI falls back to
    // scripts\Rscripts and writes RRDR.asi.log there.
    let asi_ini = root.join("native/RRDR.Asi/RRDR.asi.ini");
    if asi_ini.is_file() {
        fsutil::copy(&asi_ini, &dist_root.join("RRDR.asi.ini"))?;
    }

    let import_lib = root.join("lib/ScriptHookRDR2.lib");
    if import_lib.is_file() {
        fsutil::copy(&import_lib, &dist_root.join("RRDR.lib"))?;
    }

    let readme = root.join("docs/DIST_README.txt");
    if readme.is_file() {
        fsutil::copy(&readme, &dist_root.join("README.txt"))?;
    }

    Ok(())
}

/// Prints the package tree, sorted by path.
pub fn print(dist_root: &Path) {
    let mut files: Vec<PathBuf> = fsutil::files_recursive(dist_root, 4);
    files.sort();

    for file in files {
        let size = std::fs::metadata(&file)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        let relative = file
            .strip_prefix(dist_root)
            .unwrap_or(&file)
            .display()
            .to_string();

        crate::logger::log(
            crate::logger::LVL::Other {
                mtype: crate::logger::Mtype::Content,
            },
            format!("{:>9}  {relative}", size),
        );
    }
}
