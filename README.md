# RRDR: Rust mods for Red Dead Redemption 2

[![crates.io](https://img.shields.io/crates/v/rrdr)](https://crates.io/crates/rrdr)
[![docs](https://img.shields.io/badge/docs-noloxoyt.github.io%2FRRDR2-c2341c)](https://noloxoyt.github.io/RRDR2/)

Write Red Dead Redemption 2 mods in Rust. RRDR ships typed bindings for every game native,
a high-level entity and world API, and a native ASI loader that runs your `cdylib` in game.
No .NET required.

## Quick start

```bash
git clone --recurse-submodules https://github.com/NoloxoYT/RRDR2.git
cd RRDR2
cargo install --path build_tools --locked
rrdr
```

`rrdr` builds every mod crate, compiles `RRDR.asi`, packages `dist/RRDR/` and asks where
your game folder is. Non-interactive:

```bash
rrdr --loader asi --game-dir "C:/Games/Red Dead Redemption 2" --yes
rrdr --no-install     # build only
rrdr --help
```

Requirements: ScriptHookRDR2 + its ASI loader (`dinput8.dll`), Rust, .NET SDK, LLVM
(`clang-cl`, `lld-link`). On Linux, also `rustup target add x86_64-pc-windows-msvc` and
`cargo install cargo-xwin --locked`. Single-player only.

## A mod in ten lines

```rust
use rrdr_bridge::{export_mod, GameApi, Mod};
use std::path::Path;

struct MyMod;

impl Mod for MyMod {
    fn init(_api: &mut dyn GameApi, _config: Option<&Path>) -> Result<Self, String> {
        Ok(Self)
    }

    fn tick(&mut self, api: &mut dyn GameApi, _dt: f32) {
        let Some(player) = rrdr_game::Player::local(api) else { return };
        if let Some(ped) = player.ped_checked(api) {
            if ped.health(api) < 50 {
                ped.set_health(api, 50);
            }
        }
    }
}

export_mod!(MyMod);
```

Make the crate a `cdylib` under `rust/crates/`, run `rrdr`, and the loader picks
`MyModCore.dll` up from `scripts/Rscripts/` on the next launch.

## Documentation

- [Quick start](https://noloxoyt.github.io/RRDR2/start.html)
- [Reference](https://noloxoyt.github.io/RRDR2/reference.html): loaders, configuration, ABI, troubleshooting
- [Natives browser](https://noloxoyt.github.io/RRDR2/natives/index.html): all 7501 natives, searchable, each with its description and parameter list
- [`docs/ABI.md`](docs/ABI.md): the C ABI

## Repository

`build_tools` is a git submodule pointing at the `build_tools` branch, and the docs site
is published from the `docs` branch. See the [reference](https://noloxoyt.github.io/RRDR2/reference.html)
for the full layout.

## Branches

| Branch | Contents |
| --- | --- |
| `main` | the framework itself; `build_tools/` is a submodule |
| `build_tools` | only the `rrdr` command line tool, published to crates.io |
| `docs` | only the documentation site, published to GitHub Pages |

## License

MIT. ScriptHookRDR2 is by Alexander Blade; Red Dead Redemption 2 is a trademark of
Rockstar Games. This project is not affiliated with either. Third-party details in
[`NOTICE.md`](NOTICE.md).
