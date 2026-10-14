# Isle build helper — sets up the full MSVC + Rust environment then runs tauri.
# Usage:  .\build.ps1          # dev mode (hot reload)
#         .\build.ps1 build    # release installer
#         .\build.ps1 check    # just cargo check (faster)
param([string]$cmd = "dev")

$vcvars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
$rustBin = "$env:USERPROFILE\.cargo\bin"

if (-not (Test-Path $vcvars)) {
    Write-Error "MSVC Build Tools not found. Install from https://aka.ms/buildtools"
    exit 1
}

# Build a temp bat that sources vcvarsall then runs the requested command
$taurCmd = if ($cmd -eq "check") { "cargo check" }
           elseif ($cmd -eq "build") { "npm run tauri build" }
           else { "npm run tauri dev" }

$bat = @"
@echo off
call "$vcvars" x64
set PATH=$rustBin;%PATH%
$taurCmd
"@

$tmp = "$env:TEMP\isle_run.bat"
$bat | Set-Content $tmp -Encoding ASCII
& cmd /c $tmp
