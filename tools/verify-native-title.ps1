param(
    [string]$ShellExecutable,
    [string]$DaemonExecutable,
    [string]$ReportPath,
    [switch]$CaptureWindow
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
if ($CaptureWindow) {
    Add-Type -AssemblyName System.Drawing
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class IrisSmokeWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
}
'@
}
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
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    $captured = @{}
    $daemonIds = @()
    while (-not $process.WaitForExit(100) -and [DateTime]::UtcNow -lt $deadline) {
        if (-not $CaptureWindow) { continue }
        foreach ($stage in @('capture', 'dialog')) {
            $marker = [System.IO.Path]::ChangeExtension($reportFile, ($stage + '-ready'))
            if ($captured.ContainsKey($stage) -or -not (Test-Path -LiteralPath $marker)) { continue }
            Start-Sleep -Milliseconds 350
            if (-not $daemonIds.Count) {
                $daemonIds = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($process.Id) AND Name='iris-daemon.exe'" | Select-Object -ExpandProperty ProcessId)
                if (-not $daemonIds.Count) { throw 'Test-owned daemon was not running before background checks' }
            }
            $process.Refresh()
            $bounds = New-Object IrisSmokeWindow+Rect
            if ($process.MainWindowHandle -eq [IntPtr]::Zero -or -not [IrisSmokeWindow]::GetWindowRect($process.MainWindowHandle, [ref]$bounds)) { throw 'Cannot capture the test-owned main window' }
            $bitmap = New-Object System.Drawing.Bitmap(($bounds.Right - $bounds.Left), ($bounds.Bottom - $bounds.Top))
            $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bitmap.Size)
                $bitmap.Save(($reportFile + '.' + $stage + '.png'), [System.Drawing.Imaging.ImageFormat]::Png)
                $captured[$stage] = $true
            } finally { $graphics.Dispose(); $bitmap.Dispose() }
        }
    }
    if (-not $process.HasExited) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'Native smoke exceeded 60 seconds; only its own process tree was terminated'
    }
    $stdout.Result | Set-Content -LiteralPath ($reportFile + '.stdout.log') -Encoding utf8
    $stderr.Result | Set-Content -LiteralPath ($reportFile + '.stderr.log') -Encoding utf8
    if ($process.ExitCode -ne 0) { throw "Native smoke failed with exit $($process.ExitCode); inspect $reportFile and its logs" }
    foreach ($daemonId in $daemonIds) {
        if (Get-Process -Id $daemonId -ErrorAction SilentlyContinue) { throw 'Test-owned daemon survived application exit' }
    }
    $report = Get-Content -LiteralPath $reportFile -Raw | ConvertFrom-Json
    if (-not $report.ok -or $report.observations.Count -ne 4 -or -not $report.invalid_locale_rejected -or $report.window_checks.Count -lt 10) { throw 'Native smoke assertions failed' }
    Write-Output ('Native WebView title and window lifecycle smoke passed: ' + $reportFile)
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
