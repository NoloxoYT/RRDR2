# Build RRDR.asi with the Windows toolchain (clang-cl + lld-link).
#
# Requires LLVM (clang-cl, lld-link, llvm-readobj, llvm-nm on PATH) and either
# the Windows SDK or the headers cargo-xwin downloads. On Linux use
# tools/build_asi.sh instead, which reuses the cargo-xwin headers.
#
# IMPORTANT: this must stay in sync with tools/build_asi.sh. The link is
# CRT-less and the compiler flags are load-bearing:
#
# -fno-builtin: without it clang rewrites the hand-written copy/compare helpers
#   in RRDR.Asi.cpp into memcpy/strlen/strrchr calls, which pulls
#   api-ms-win-crt-*.dll imports. On Wine a failed import makes the ASI fail to
#   load without a single log line.
# /entry:RustDllEntry /include:RustDllEntry: the linker is driven without a
#   default entry (/noentry) and treats every COMDAT as unused, dropping
#   DllMain. That once produced a 12 KB DLL with no code, no strings and no
#   entry work at all: the loader "loaded" and did nothing.
# No /EHsc: C++ unwinding would need __CxxFrameHandler3 from the CRT, which the
#   nodefaultlib link cannot satisfy.

param(
    [string]$Out = "dist/RRDR/RRDR.asi",
    [string]$XwinDir = "$env:USERPROFILE\.cache\cargo-xwin\xwin"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    $src = "native/RRDR.Asi/RRDR.Asi.cpp"
    if (-not (Test-Path $src)) {
        throw "Missing $src"
    }

    foreach ($tool in @("clang-cl", "lld-link")) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "$tool not found. Install LLVM (or Visual Studio Build Tools with the C++ workload)."
        }
    }

    if (-not (Test-Path "$XwinDir/sdk/include/um")) {
        Write-Warning "Windows SDK headers missing in $XwinDir - compiling with whatever is on INCLUDE. Run 'cargo xwin build' once to download them into $XwinDir."
    }

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Out) | Out-Null

    $obj = Join-Path ([System.IO.Path]::GetTempPath()) "RRDR.Asi.obj"

    # Same flags as build_asi.sh (see the comment at the top of this file).
    & clang-cl --target=x86_64-pc-windows-msvc /O2 /fms-extensions /fno-builtin /c $src /Fo"$obj"
    if ($LASTEXITCODE -ne 0) {
        throw "clang-cl failed"
    }

    # The CRT libs come from the cargo-xwin download when available; fall back
    # to the SDK ones on PATH. The link is /nodefaultlib either way.
    $crtLibs = @()
    if (Test-Path "$XwinDir/crt/lib/x86_64/libcmt.lib") {
        $crtLibs += "$XwinDir/crt/lib/x86_64/libcmt.lib",
                    "$XwinDir/crt/lib/x86_64/libcpmt.lib",
                    "$XwinDir/crt/lib/x86_64/libvcruntime.lib"
    }
    $sdkLibs = @()
    if (Test-Path "$XwinDir/sdk/lib/ucrt/x86_64/ucrt.lib") {
        $sdkLibs += "$XwinDir/sdk/lib/ucrt/x86_64/ucrt.lib",
                    "$XwinDir/sdk/lib/um/x86_64/kernel32.lib",
                    "$XwinDir/sdk/lib/um/x86_64/user32.lib"
    }
    else {
        # No cargo-xwin download: rely on the SDK libs on the LIB path.
        $sdkLibs += "kernel32.lib", "user32.lib"
    }

    & lld-link /dll /machine:x64 /nodefaultlib /force:unresolved `
        /entry:RustDllEntry /include:RustDllEntry `
        /out:"$Out" @crtLibs @sdkLibs $obj
    if ($LASTEXITCODE -ne 0) {
        throw "lld-link failed"
    }

    # Post-link checks, matching build_asi.sh: the import table must only
    # contain the DLLs we resolve by hand, and any undefined symbol kept by
    # /force:unresolved is a call-to-null waiting on a code path.
    $readobj = Get-Command llvm-readobj -ErrorAction SilentlyContinue
    if ($readobj) {
        $imports = (& llvm-readobj --coff-imports $Out | Select-String '^  Name:').Line
        $imports = $imports -replace '^  Name: ', ''
        Write-Host "imports: $($imports -join ' ')"
        foreach ($dll in $imports) {
            if ($dll -notmatch '^(?i)kernel32\.dll$|^(?i)user32\.dll$') {
                throw "$Out imports $dll (expected only KERNEL32/USER32: a CRT call leaked back in)"
            }
        }
    }

    Remove-Item $obj -Force -ErrorAction SilentlyContinue

    # The loader reads <game>\RRDR.asi.ini next to RDR2.exe (defaults:
    # mods_dir=scripts\Rscripts, reload_config=true). Copy it when packaging.
    $iniSource = "native/RRDR.Asi/RRDR.asi.ini"
    $iniTarget = Join-Path (Split-Path -Parent $Out) "RRDR.asi.ini"
    if ((Test-Path $iniSource) -and -not (Test-Path $iniTarget)) {
        Copy-Item $iniSource $iniTarget
        Write-Host "copied RRDR.asi.ini next to the ASI"
    }

    Write-Host "wrote $Out"
}
finally {
    Pop-Location
}
