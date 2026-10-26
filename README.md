# rrdr

Build and install [RRDR](https://noloxoyt.github.io/RRDR2/), the Rust mod framework for
Red Dead Redemption 2: every mod crate, the managed host, the native `RRDR.asi` loader,
then an optional install into your game folder.

## Install

```bash
cargo install rrdr          # from crates.io
cargo install --path build_tools --locked   # from a checkout
```

## Use

Run it inside a checkout of the RRDR repository:

```bash
rrdr                                   # build, then install interactively
rrdr --no-install                      # build only, package in dist/RRDR/
rrdr --loader asi --game-dir "C:/Games/Red Dead Redemption 2" --yes
rrdr --root ~/RRDR2 --loader host      # checkout somewhere else
rrdr --skip-tests --skip-sim           # faster iteration
```

The checkout is found from `--root`, then `$RRDR_ROOT`, then the current directory and its
parents. The game folder is found from `--game-dir`, then `$RDR2_DIR`, then the usual install
locations.

## New mod

`rrdr init` works like `cargo init` and creates a ready-to-build mod crate:

```bash
rrdr init hunt              # rust/crates/hunt, DLL HuntCore.dll, added to the workspace
rrdr init hunt --force      # rewrite the template in an existing folder
rrdr init my-mod --path .   # crate in ./my-mod, outside a checkout
```

Inside a checkout the crate goes to `rust/crates/<name>` and is registered in
`rust/Cargo.toml`. The template ships a `Mod` implementation, a config file and a
`.gitignore`; the next `rrdr` run builds and packages it.

| Step | Command it runs |
| --- | --- |
| New mod | `rrdr init <name>` writes a crate and a workspace entry |
| Rust tests | `cargo test --workspace` |
| Debug cores | `cargo build` |
| Simulator | `dotnet run` on `RRDR.Sim` |
| Windows cores | `cargo xwin build` (native `cargo build` on Windows) |
| Managed host | `dotnet build` on `RRDR.Host`, net48 |
| Native loader | `clang-cl` + `lld-link` on `RRDR.Asi.cpp` |
| Package | `dist/RRDR/` |

Missing tools are reported before the long steps start.

## License

MIT
