param(
    [string]$ShellExecutable,
    [string]$DaemonExecutable,
    [string]$ReportPath
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $ShellExecutable) { $ShellExecutable = Join-Path $repositoryRoot 'apps/shell/src-tauri/target/debug/iris-shell.exe' }
if (-not $DaemonExecutable) { $DaemonExecutable = Join-Path $repositoryRoot 'target/debug/iris-daemon.exe' }
if (-not $ReportPath) { $ReportPath = Join-Path $repositoryRoot ('artifacts/native-title-' + [guid]::NewGuid().ToString('N') + '.json') }
$reportFile = [System.IO.Path]::GetFullPath($ReportPath)
if (Test-Path -LiteralPath $reportFile) { throw 'Choose a new smoke report path; existing reports are preserved' }
$shellFile = (Resolve-Path -LiteralPath $ShellExecutable).Path
$daemonFile = (Resolve-Path -LiteralPath $DaemonExecutable).Path
$temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$smokeRoot = Join-Path $temporaryRoot ('iris-native-title-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $smokeRoot | Out-Null
$process = $null
try {
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $shellFile
    $start.ArgumentList.Add('--verify-native-title')
    $start.ArgumentList.Add($reportFile)
    $start.WorkingDirectory = $smokeRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment['IRIS_DAEMON_PATH'] = $daemonFile
    $start.Environment['IRIS_MODEL_DIR'] = Join-Path $repositoryRoot 'models'
    $start.Environment['IRIS_DATA_DIR'] = Join-Path $smokeRoot 'data'
    $start.Environment['WEBVIEW2_USER_DATA_FOLDER'] = Join-Path $smokeRoot 'webview'
    $start.Environment.Remove('ORT_DYLIB_PATH') | Out-Null
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $start
    if (-not $process.Start()) { throw 'Native smoke application failed to start' }
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(65000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'Native smoke exceeded 65 seconds; only its own process tree was terminated'
    }
    $stdout.Result | Set-Content -LiteralPath ($reportFile + '.stdout.log') -Encoding utf8
    $stderr.Result | Set-Content -LiteralPath ($reportFile + '.stderr.log') -Encoding utf8
    if ($process.ExitCode -ne 0) { throw "Native smoke failed with exit $($process.ExitCode); inspect $reportFile and its logs" }
    $report = Get-Content -LiteralPath $reportFile -Raw | ConvertFrom-Json
    if (-not $report.ok -or $report.observations.Count -ne 4 -or -not $report.invalid_locale_rejected) { throw 'Native smoke assertions failed' }
    Write-Output ('Native WebView title smoke passed: ' + $reportFile)
} finally {
    if ($null -ne $process -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
    if ($null -ne $process) { $process.Dispose() }
    $resolvedSmoke = [System.IO.Path]::GetFullPath($smokeRoot)
    $expectedPrefix = Join-Path $temporaryRoot 'iris-native-title-'
    if (-not $resolvedSmoke.StartsWith($expectedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing cleanup outside the test-owned temp directory' }
    # WebView2 may release its test-owned profile just after the application exits.
    for ($cleanupAttempt = 0; $cleanupAttempt -lt 20; $cleanupAttempt++) {
        try { Remove-Item -LiteralPath $resolvedSmoke -Recurse -Force; break }
        catch { if ($cleanupAttempt -eq 19) { throw }; Start-Sleep -Milliseconds 250 }
    }
}
