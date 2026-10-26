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

| Step | Command it runs |
| --- | --- |
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
