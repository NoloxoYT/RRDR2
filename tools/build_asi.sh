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

echo "wrote $OUT"
