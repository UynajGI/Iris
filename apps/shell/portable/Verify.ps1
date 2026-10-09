param([string]$ReportPath = (Join-Path $PSScriptRoot 'verification.json'))
$ErrorActionPreference = 'Stop'
$bundleRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
if (Test-Path -LiteralPath (Join-Path $bundleRoot 'models/optional')) { throw 'General portable bundles must not contain user-supplied optional models' }
$manifest = Get-Content -LiteralPath (Join-Path $bundleRoot 'checksums.json') -Raw | ConvertFrom-Json
foreach ($entry in $manifest.files) {
    if ($entry.path -match '^models[/\\]optional([/\\]|$)') { throw 'Optional model entry found in package manifest' }
    $file = [System.IO.Path]::GetFullPath((Join-Path $bundleRoot $entry.path))
    if (-not $file.StartsWith($bundleRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid checksum manifest path' }
    if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256) { throw "Checksum mismatch: $($entry.path)" }
}
$temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$verificationRoot = Join-Path $temporaryRoot ('iris-portable-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $verificationRoot | Out-Null
$daemon = $null
$environmentNames = @('IRIS_DATA_DIR', 'IRIS_DAEMON_PATH', 'IRIS_MODEL_DIR', 'ORT_DYLIB_PATH')
try {
    # Exercise installed-sibling model resolution, without repository environment overrides.
    $hostReport = Join-Path $verificationRoot 'host.json'
    $hostStart = New-Object System.Diagnostics.ProcessStartInfo
    $hostStart.FileName = Join-Path $bundleRoot 'iris-shell.exe'
    $hostStart.Arguments = '--verify-host "' + $hostReport + '"'
    $hostStart.WorkingDirectory = $verificationRoot
    $hostStart.UseShellExecute = $false
    $hostStart.CreateNoWindow = $true
    $hostStart.RedirectStandardError = $true
    foreach ($name in $environmentNames) { $hostStart.Environment.Remove($name) | Out-Null }
    $hostStart.Environment['IRIS_DATA_DIR'] = Join-Path $verificationRoot 'host-data'
    $hostProcess = New-Object System.Diagnostics.Process
    $hostProcess.StartInfo = $hostStart
    if (-not $hostProcess.Start()) { throw 'Packaged host did not start' }
    $hostError = $hostProcess.StandardError.ReadToEndAsync()
    if (-not $hostProcess.WaitForExit(40000)) { $hostProcess.Kill(); throw 'Packaged host verification timed out' }
    if ($hostProcess.ExitCode -ne 0) { throw "Packaged host exited with $($hostProcess.ExitCode): $($hostError.Result)" }
    $hostResult = Get-Content -LiteralPath $hostReport -Raw | ConvertFrom-Json
    if (-not $hostResult.ok) { throw 'Packaged host heartbeat failed' }

    # A synthetic photograph verifies model/runtime loading without shipping user photos.
    Add-Type -AssemblyName System.Drawing
    $photos = Join-Path $verificationRoot 'synthetic'
    New-Item -ItemType Directory -Path $photos | Out-Null
    $bitmap = New-Object System.Drawing.Bitmap 256,256
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.Clear([System.Drawing.Color]::FromArgb(128,128,128))
    $graphics.Dispose()
    $bitmap.Save((Join-Path $photos 'synthetic.jpg'), [System.Drawing.Imaging.ImageFormat]::Jpeg)
    $bitmap.Dispose()
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = Join-Path $bundleRoot 'iris-daemon.exe'
    $start.Arguments = '--data-dir "' + (Join-Path $verificationRoot 'analysis-data') + '" --model-dir "' + (Join-Path $bundleRoot 'models') + '" --port 0'
    $start.WorkingDirectory = $verificationRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($name in $environmentNames) { $start.Environment.Remove($name) | Out-Null }
    $daemon = New-Object System.Diagnostics.Process
    $daemon.StartInfo = $start
    if (-not $daemon.Start()) { throw 'Packaged daemon did not start' }
    $stderr = $daemon.StandardError.ReadToEndAsync()
    $bootstrapLine = $daemon.StandardOutput.ReadLineAsync()
    if (-not $bootstrapLine.Wait(30000)) { throw 'Packaged daemon startup timeout' }
    $session = $bootstrapLine.Result | ConvertFrom-Json
    $headers = @{ Authorization = 'Bearer ' + $session.token }
    $api = $session.base_url + '/api/v1'
    $project = Invoke-RestMethod -Uri ($api + '/projects') -Method Post -Headers $headers -ContentType 'application/json' -Body (@{root=$photos} | ConvertTo-Json)
    foreach ($operation in @('scan','analyze')) {
        Invoke-RestMethod -Uri ($api + '/projects/' + $project.id + '/' + $operation) -Method Post -Headers $headers | Out-Null
        $deadline = [DateTime]::UtcNow.AddSeconds(90)
        do {
            Start-Sleep -Milliseconds 100
            $progress = Invoke-RestMethod -Uri ($api + '/projects/' + $project.id + '/progress') -Headers $headers
            if ([DateTime]::UtcNow -gt $deadline) { throw "Packaged $operation timeout" }
        } while ($progress.state -in @('running','paused'))
        if ($progress.state -ne 'completed' -or $progress.errors.Count -gt 0) { throw "Packaged $operation failed: $($progress.errors -join '; ')" }
    }
    $analyzed = @(Invoke-RestMethod -Uri ($api + '/projects/' + $project.id + '/photos') -Headers $headers)
    if ($analyzed.Count -ne 1 -or $null -eq $analyzed[0].analysis) { throw 'Packaged analysis did not persist a result' }
    $daemon.StandardInput.WriteLine('shutdown')
    if (-not $daemon.WaitForExit(10000)) { throw 'Packaged daemon did not shut down gracefully' }
    if ($daemon.ExitCode -ne 0) { throw 'Packaged daemon shutdown failed' }
    $result = [ordered]@{ok=$true; checksummed_files=$manifest.files.Count; outside_repository_working_directory=$true; host_heartbeat=$hostResult.heartbeat; models_available=$hostResult.files; synthetic_jpg_analyzed=$true; source_fixtures_included=$false; optional_models_included=$false}
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath ([System.IO.Path]::GetFullPath($ReportPath)) -Encoding utf8
    Write-Output ('Portable verification passed: ' + [System.IO.Path]::GetFullPath($ReportPath))
} finally {
    if ($null -ne $daemon -and -not $daemon.HasExited) { $daemon.StandardInput.WriteLine('shutdown'); if (-not $daemon.WaitForExit(5000)) { $daemon.Kill() } }
    $resolvedVerification = [System.IO.Path]::GetFullPath($verificationRoot)
    if (-not $resolvedVerification.StartsWith($temporaryRoot, [System.StringComparison]::OrdinalIgnoreCase) -or $resolvedVerification -eq $temporaryRoot) { throw 'Refusing temporary cleanup outside temp directory' }
    Remove-Item -LiteralPath $resolvedVerification -Recurse -Force
}
