# Third-party notices

## ScriptHookRDRNetAPI.dll

`lib/ScriptHookRDRNetAPI.dll` is the developer API of
[ScriptHookRDR2DotNet V2](https://github.com/Halen84/ScriptHookRDR2DotNet-V2),
licensed under the zlib license. See `lib/LICENSE-ScriptHookRDR2DotNet.txt`.

It is included only as a build reference for the `net48` host. It is not
redistributed as part of the `dist` mod package; ScriptHookRDR2DotNet installs
its own copy in the game folder.

## NativeDecl.cs

`lib/NativeDecl.cs` is the generated native declaration file from
[ScriptHookRDR2DotNet V2](https://github.com/Halen84/ScriptHookRDR2DotNet-V2),
licensed under the zlib license. It is used by `tools/generate_natives.py` to
produce `rust/crates/rrdr-natives`; the generated file is not hand-edited.

## ScriptHookRDR2 / Red Dead Redemption 2

ScriptHookRDR2 is by Alexander Blade. Red Dead Redemption 2 is a trademark of
Rockstar Games. This project is not affiliated with either.

## Rust crates

The Rust workspace has no third-party crate dependencies; it uses only `std`.
