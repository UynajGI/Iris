param(
    [string]$DaemonExecutable,
    [string]$ReportPath,
    [ValidateRange(24,64)][int]$PhotoCount = 36
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $DaemonExecutable) { $DaemonExecutable = Join-Path $repositoryRoot 'target/debug/iris-daemon.exe' }
if (-not $ReportPath) { $ReportPath = Join-Path $repositoryRoot 'artifacts/update-active-workload.json' }
$daemon = (Resolve-Path -LiteralPath $DaemonExecutable).Path
$report = [System.IO.Path]::GetFullPath($ReportPath)
$temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$fixture = Join-Path $temporaryRoot ('iris-update-active-photos-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$saved = @{}
foreach ($name in @('IRIS_TEST_DAEMON','IRIS_TEST_PHOTOS','IRIS_TEST_UPDATE_REPORT')) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
Push-Location $repositoryRoot
try {
    Add-Type -AssemblyName System.Drawing
    for ($index = 0; $index -lt $PhotoCount; $index++) {
        $bitmap = [System.Drawing.Bitmap]::new(2048,1536)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.Clear([System.Drawing.Color]::FromArgb(90 + ($index % 80),110,130))
            $random = [Random]::new(1024 + $index)
            for ($line = 0; $line -lt 240; $line++) {
                $pen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb($random.Next(256),$random.Next(256),$random.Next(256)), $random.Next(2,18))
                try { $graphics.DrawLine($pen,$random.Next(2048),$random.Next(1536),$random.Next(2048),$random.Next(1536)) }
                finally { $pen.Dispose() }
            }
            $bitmap.Save((Join-Path $fixture ('synthetic-{0:D3}.jpg' -f $index)),[System.Drawing.Imaging.ImageFormat]::Jpeg)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    }
    $env:IRIS_TEST_DAEMON = $daemon
    $env:IRIS_TEST_PHOTOS = $fixture
    $env:IRIS_TEST_UPDATE_REPORT = $report
    & cargo test --manifest-path apps/shell/src-tauri/Cargo.toml --features updater-client --lib --locked real_active_analysis_workers_exit_before_update_and_database_recovers -- --ignored
    if ($LASTEXITCODE -ne 0) { throw "Active workload update shutdown test failed; report: $report" }
    $result = Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
    if (-not $result.ok -or $result.forced_cleanup -or $result.installer_invoked) { throw 'Active workload verification did not satisfy its safety assertions' }
    Write-Output ('Verified active analysis shutdown and recovery: ' + $report)
} finally {
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
    Pop-Location
    $resolvedFixture = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $fixture).Path)
    if (-not $resolvedFixture.StartsWith($temporaryRoot, [System.StringComparison]::OrdinalIgnoreCase) -or $resolvedFixture -eq $temporaryRoot -or -not ([System.IO.Path]::GetFileName($resolvedFixture)).StartsWith('iris-update-active-photos-')) { throw 'Refusing cleanup outside owned temporary fixture' }
    Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
}
