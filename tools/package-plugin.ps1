Param(
  [string]$CrateDir = "moq_ffi",
  [string]$OutDir = "artifacts/plugin-windows-x64",
  [string]$ThirdPartyName = "moq_ffi"
)

$ErrorActionPreference = "Stop"

function Info($msg) { Write-Host $msg -ForegroundColor Cyan }
function Warn($msg) { Write-Warning $msg }
function Die($msg)  { Write-Error $msg; exit 1 }

$RepoRoot = Split-Path -Parent $PSCommandPath
$RepoRoot = Split-Path -Parent $RepoRoot  # tools -> repo root

if (-not (Test-Path $CrateDir)) { $CrateDir = Join-Path $RepoRoot $CrateDir }
if (-not (Test-Path $CrateDir)) { Die "[package-plugin] Crate directory not found: $CrateDir" }

$TargetDir = Join-Path $CrateDir "target\release"
if (-not (Test-Path $TargetDir)) { Die "[package-plugin] Target dir not found: $TargetDir (build first)" }

$IncludeSrc = Join-Path $CrateDir "include"
if (-not (Test-Path $IncludeSrc)) { Die "[package-plugin] Include dir not found: $IncludeSrc" }

# Layout (drop-in for UE plugin):
# artifacts/plugin-windows-x64/
#   ThirdParty/moq_ffi/include/*.h
#   ThirdParty/moq_ffi/lib/Win64/Release/{moq_ffi.dll.lib, moq_ffi.lib}
#   ThirdParty/moq_ffi/bin/Win64/Release/{moq_ffi.dll, moq_ffi.pdb}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$OutThirdParty = Join-Path $OutDir (Join-Path "ThirdParty" $ThirdPartyName)
$OutInclude    = Join-Path $OutThirdParty "include"
$OutLib        = Join-Path $OutThirdParty "lib\Win64\Release"
$OutBin        = Join-Path $OutThirdParty "bin\Win64\Release"

New-Item -ItemType Directory -Force -Path $OutInclude | Out-Null
New-Item -ItemType Directory -Force -Path $OutLib | Out-Null
New-Item -ItemType Directory -Force -Path $OutBin | Out-Null

# Ensure no stale static libs remain from previous runs
Remove-Item -ErrorAction SilentlyContinue (Join-Path $OutLib "moq_ffi.lib")

Info "[package-plugin] Copying headers -> $OutInclude"
Copy-Item (Join-Path $IncludeSrc "*.h") -Destination $OutInclude -Force

Info "[package-plugin] Copying libs -> $OutLib (import lib only)"
$implib = Join-Path $TargetDir "moq_ffi.dll.lib"
if (Test-Path $implib) { Copy-Item $implib -Destination $OutLib -Force } else { Warn "[package-plugin] Missing $implib" }

Info "[package-plugin] Copying binaries -> $OutBin"
$dll = Join-Path $TargetDir "moq_ffi.dll"
$pdb = Join-Path $TargetDir "moq_ffi.pdb"
if (Test-Path $dll) { Copy-Item $dll -Destination $OutBin -Force } else { Warn "[package-plugin] Missing $dll" }
if (Test-Path $pdb) { Copy-Item $pdb -Destination $OutBin -Force } else { Warn "[package-plugin] Missing $pdb" }

Info "[package-plugin] Done -> $OutDir"
