//! Command line interface.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Which loader the installer sets up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Loader {
    /// Native `RRDR.asi`. No .NET needed. Recommended.
    Asi,
    /// Managed `RRDR.Host.dll`. Needs ScriptHookRDR2DotNet.
    Host,
}

#[derive(Debug, Parser)]
#[command(
    name = "rrdr",
    version,
    about = "Build RRDR and install it into Red Dead Redemption 2",
    long_about = "Builds every Rust mod crate, the managed host and the native RRDR.asi, \
                  assembles dist/RRDR/ and offers to install it into your game folder.\n\n\
                  Run it inside an RRDR checkout, or point --root at one.",
    after_help = "EXAMPLES:\n  \
                  rrdr                                     build, then install interactively\n  \
                  rrdr --no-install                        build only, package in dist/RRDR/\n  \
                  rrdr --loader asi --game-dir \"C:/Games/RDR2\" --yes\n  \
                  rrdr --root ~/RRDR2 --loader host"
)]
pub struct Args {
    /// RRDR checkout to build. Defaults to $RRDR_ROOT, then the current directory and its parents.
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Game folder containing RDR2.exe. Skips the prompt.
    #[arg(long, value_name = "DIR")]
    pub game_dir: Option<PathBuf>,

    /// Loader to install. Skips the prompt.
    #[arg(long, value_enum, value_name = "LOADER")]
    pub loader: Option<Loader>,

    /// Install without asking for confirmation.
    #[arg(short, long)]
    pub yes: bool,

    /// Build only; never touch the game folder.
    #[arg(long)]
    pub no_install: bool,

    /// Skip the Rust unit tests.
    #[arg(long)]
    pub skip_tests: bool,

    /// Skip the end-to-end simulator.
    #[arg(long)]
    pub skip_sim: bool,
}
