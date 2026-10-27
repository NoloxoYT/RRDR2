#!/usr/bin/env bash
# Build RRDR.asi, the native ScriptHookRDR2 loader.
#
# Uses clang-cl + lld-link from the host and the Windows SDK/CRT headers that
# cargo-xwin already downloaded. No Visual Studio install is required.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SRC="native/RRDR.Asi/RRDR.Asi.cpp"
OUT="${1:-dist/RRDR/RRDR.asi}"
XWIN="${XWIN_DIR:-$HOME/.cache/cargo-xwin/xwin}"

if [ ! -f "$SRC" ]; then
    echo "missing $SRC" >&2
    exit 1
fi

for tool in clang-cl lld-link; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "$tool not found (install llvm/clang)" >&2
        exit 1
    fi
done

if [ ! -d "$XWIN/sdk/include/um" ]; then
    echo "Windows SDK headers missing in $XWIN" >&2
    echo "run a cargo xwin build once to download them" >&2
    exit 1
fi

OBJ="$(mktemp -d)/RRDR.Asi.obj"
trap 'rm -rf "$(dirname "$OBJ")"' EXIT

# Note: the CRT-less link below discards COMDATs it considers unused. That once
# produced a 12 KB DLL with no code, no strings and no entry work at all.
#
# -fno-builtin: without it clang rewrites the hand-written copy/compare helpers
# into memcpy/strlen/strrchr calls, which pulls api-ms-win-crt-*.dll imports.
# On Wine a failed import makes the ASI load fail without a single log line.
clang-cl --target=x86_64-pc-windows-msvc -O2 -fms-extensions -fno-builtin \
    -imsvc "$XWIN/crt/include" \
    -imsvc "$XWIN/sdk/include/ucrt" \
    -imsvc "$XWIN/sdk/include/shared" \
    -imsvc "$XWIN/sdk/include/um" \
    /c "$SRC" /Fo"$OBJ"

mkdir -p "$(dirname "$OUT")"

# /force:unresolved: the loader is linked without a C runtime, so libcmt brings
# obj files that reference unwind helpers and stack probes the nodefaultlib link
# cannot satisfy. They sit behind disabled paths; letting the linker through is
# what keeps the module intact.
lld-link /dll /machine:x64 /nodefaultlib /force:unresolved \
    /entry:RustDllEntry /include:RustDllEntry \
    /out:"$OUT" \
    "$XWIN/crt/lib/x86_64/libcmt.lib" \
    "$XWIN/crt/lib/x86_64/libcpmt.lib" \
    "$XWIN/crt/lib/x86_64/libvcruntime.lib" \
    "$XWIN/sdk/lib/ucrt/x86_64/ucrt.lib" \
    "$XWIN/sdk/lib/um/x86_64/kernel32.lib" \
    "$XWIN/sdk/lib/um/x86_64/user32.lib" \
    "$OBJ"

# Post-link check (the link above is deliberately permissive):
#
# 1. The import table must only contain the DLLs we resolved by hand. Anything
#    else (api-ms-win-crt-*, vcruntime, ...) means a CRT call sneaked back in,
#    which fails to load on Wine with no message at all.
# 2. Undefined symbols left as call targets by /force:unresolved are listed as
#    a warning: each one is a call-to-null waiting on a code path being hit.
if command -v llvm-readobj >/dev/null 2>&1; then
    imports="$(llvm-readobj --coff-imports "$OUT" 2>/dev/null | grep -E '^  Name:' | sed 's/.*Name: //')"
    echo "imports: $(echo "$imports" | tr '\n' ' ')"
    for dll in $imports; do
        case "$(echo "$dll" | tr 'A-Z' 'a-z')" in
            kernel32.dll|user32.dll) ;;
            *)
                echo "ERROR: $OUT imports $dll (expected only KERNEL32/USER32: a CRT call leaked back in)" >&2
                exit 1
                ;;
        esac
    done
fi

if command -v llvm-nm >/dev/null 2>&1; then
    unresolved="$(llvm-nm "$OUT" 2>/dev/null | grep -E ' [Uw] ' || true)"
    if [ -n "$unresolved" ]; then
        echo "warning: unresolved symbols kept by /force:unresolved:" >&2
        echo "$unresolved" >&2
    fi
fi

# The loader reads <game>/RRDR.asi.ini next to RDR2.exe (not this repo copy):
#   [RRDR]
#   mods_dir = scripts\\Rscripts   ; absolute paths allowed, both / and \\
#   reload_config = true
# dist/ packaging copies native/RRDR.Asi/RRDR.asi.ini in; a hand build does not,
# so remind the builder that the defaults live there.
if [ -f "native/RRDR.Asi/RRDR.asi.ini" ] && [ ! -f "$(dirname "$OUT")/RRDR.asi.ini" ]; then
    echo "note: native/RRDR.Asi/RRDR.asi.ini was not copied next to the ASI; the loader uses its defaults (mods_dir=scripts\\\\Rscripts, reload_config=true)"
fi

echo "wrote $OUT"
