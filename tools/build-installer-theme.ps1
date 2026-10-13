# SPDX-License-Identifier: GPL-3.0-or-later
param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'The installer theme requires the Visual Studio C++ x86 build tools.' }
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $installation) { throw 'Cannot locate the Visual Studio C++ x86 build tools.' }
$vcvars = Join-Path $installation 'VC/Auxiliary/Build/vcvarsall.bat'
$source = Join-Path $root 'apps/shell/installer/theme/theme.cpp'
$exports = Join-Path $root 'apps/shell/installer/theme/theme.def'
& (Join-Path $PSScriptRoot 'compile-installer-theme.cmd') $vcvars $source $output $exports
if ($LASTEXITCODE -ne 0) { throw 'Cannot compile the NSIS x86 Unicode theme plug-in.' }
$dll = Join-Path $output 'IrisTheme.dll'
$bytes = [IO.File]::ReadAllBytes($dll)
$pe = [BitConverter]::ToInt32($bytes, 0x3c)
if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x14c) { throw 'NSIS requires an x86 theme DLL, including for x64 applications.' }
Write-Output $dll
