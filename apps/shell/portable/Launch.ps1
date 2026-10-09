param([string]$DataDirectory = (Join-Path $env:LOCALAPPDATA 'IrisVision'))
$ErrorActionPreference = 'Stop'
$bundleRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
$env:IRIS_DAEMON_PATH = Join-Path $bundleRoot 'iris-daemon.exe'
$env:IRIS_MODEL_DIR = Join-Path $bundleRoot 'models'
$env:ORT_DYLIB_PATH = Join-Path $env:IRIS_MODEL_DIR 'onnxruntime.dll'
$env:IRIS_DATA_DIR = [System.IO.Path]::GetFullPath($DataDirectory)
New-Item -ItemType Directory -Force -Path $env:IRIS_DATA_DIR | Out-Null
Start-Process -FilePath (Join-Path $bundleRoot 'iris-shell.exe') -WorkingDirectory $bundleRoot -WindowStyle Hidden -PassThru
