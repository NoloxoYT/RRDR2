# Build RRDR.asi with the Windows toolchain (clang-cl + lld-link).
#
# Requires Visual Studio Build Tools (or clang-cl on PATH) and the Windows SDK.
# On Linux use tools/build_asi.sh instead, which reuses the headers downloaded
# by cargo-xwin.

param(
    [string]$Out = "dist/RRDR/RRDR.asi"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    $src = "native/RRDR.Asi/RRDR.Asi.cpp"
    if (-not (Test-Path $src)) {
        throw "Missing $src"
    }

    $clang = Get-Command clang-cl -ErrorAction SilentlyContinue
    if (-not $clang) {
        throw "clang-cl not found. Install Visual Studio Build Tools with the C++ workload."
    }

    $link = Get-Command lld-link -ErrorAction SilentlyContinue
    if (-not $link) {
        throw "lld-link not found. Install LLVM (it ships with lld)."
    }

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Out) | Out-Null

    $obj = Join-Path ([System.IO.Path]::GetTempPath()) "RRDR.Asi.obj"

    & clang-cl /c $src /Fo$obj /O2 /EHsc
    if ($LASTEXITCODE -ne 0) {
        throw "clang-cl failed"
    }

    & lld-link /dll /noentry /machine:x64 /out:$Out $obj kernel32.lib user32.lib
    if ($LASTEXITCODE -ne 0) {
        throw "lld-link failed"
    }

    Remove-Item $obj -Force -ErrorAction SilentlyContinue
    Write-Host "wrote $Out"
}
finally {
    Pop-Location
}
