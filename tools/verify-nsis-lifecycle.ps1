#Requires -Version 7.0
param(
    [string]$SourceBuildReport = 'dist/installers/IrisVision-0.1.0-windows-x64-v5-functional-final/build-report.json',
    [string]$PriorHarnessFailureReport,
    [switch]$VerifyRawFallback
)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -lt 7) { throw 'PowerShell 7 is required for process-tree timeout cleanup' }
if (-not [Environment]::Is64BitProcess) { throw 'Run this verifier from 64-bit PowerShell' }
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not [IO.Path]::IsPathRooted($SourceBuildReport)) { $SourceBuildReport = Join-Path $repositoryRoot $SourceBuildReport }
$sourceReport = Get-Content -LiteralPath $SourceBuildReport -Raw | ConvertFrom-Json
$sourceNsi = Join-Path $sourceReport.stage 'target/release/nsis/x64/installer.nsi'
$sourceUtils = Join-Path (Split-Path -Parent $sourceNsi) 'utils.nsh'
$sourceShell = Join-Path $sourceReport.stage 'target/release/iris-shell.exe'
$sourceInstaller = Join-Path (Split-Path -Parent $SourceBuildReport) $sourceReport.installer
$compiler = Join-Path $env:LOCALAPPDATA 'tauri/NSIS/makensis.exe'
foreach ($path in @($sourceNsi,$sourceUtils,$sourceShell,$sourceInstaller,$compiler)) { if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing prerequisite: $path" } }
$nonce = [Guid]::NewGuid().ToString('N')
$product = 'IrisVision-QA-' + $nonce
$publisher = 'IrisVision-QA-Publisher-' + $nonce
$bundleId = 'local.irisvision.qa.run' + $nonce
$binaryName = 'iris-qa-' + $nonce
$qaRoot = Join-Path $repositoryRoot ('artifacts/nsis-qa-' + $nonce)
$installRoot = Join-Path $qaRoot 'installed'
$dataRoot = Join-Path $qaRoot 'data'
$photosRoot = Join-Path $qaRoot 'photos'
$sourceRoot = Join-Path $qaRoot 'inputs'
$reportPath = Join-Path $qaRoot 'lifecycle-report.json'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\' + $product
$publisherKey = 'HKCU:\Software\' + $publisher
$productKey = $publisherKey + '\' + $product
$desktop = [Environment]::GetFolderPath('DesktopDirectory')
$programs = [Environment]::GetFolderPath('Programs')
$qaShortcuts = @((Join-Path $desktop ($product + '.lnk')),(Join-Path $programs ($product + '.lnk')))
$report = [ordered]@{
    ok=$false; namespace=$nonce; product_name=$product; publisher=$publisher; bundle_id=$bundleId; main_binary_name=$binaryName
    qa_root=$qaRoot; install_root=$installRoot; data_root=$dataRoot
    source_build_report=[IO.Path]::GetFullPath($SourceBuildReport)
    payload_application_version=$sourceReport.version; qa_installer_versions=@('0.1.0','0.1.1')
    real_application_version_migration_tested=$false; updater_install_e2e_tested=$false
    normal_tauri_started=$false; final_delivery_artifacts_modified=$false; steps=@(); cleanup_complete=$false
    harness_environment_clear='Remove-Item Env: performs actual deletion; SetEnvironmentVariable(name,$null) can create empty overrides on this host'
    prior_harness_failure_report=$PriorHarnessFailureReport
}
function Assert-QaPath([string]$Path) {
    $resolved = [IO.Path]::GetFullPath($Path)
    if (-not $resolved.StartsWith($qaRoot.TrimEnd('\') + '\',[StringComparison]::OrdinalIgnoreCase)) { throw "Path escapes unique QA root: $resolved" }
    return $resolved
}
function Registry-Snapshot([string]$Key) {
    if (-not (Test-Path -LiteralPath $Key)) { return [ordered]@{exists=$false} }
    $item = Get-Item -LiteralPath $Key
    # An array preserves the registry's unnamed/default value without producing
    # an empty JSON property name that PowerShell's object parser cannot read.
    $values = @()
    foreach ($name in ($item.GetValueNames() | Sort-Object)) { $values += [ordered]@{name=$name; kind=$item.GetValueKind($name).ToString(); value=$item.GetValue($name)} }
    return [ordered]@{exists=$true; values=$values; children=@($item.GetSubKeyNames() | Sort-Object)}
}
function Real-State {
    $keys = [ordered]@{}
    foreach ($key in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\IrisVision','HKCU:\Software\irisvision\IrisVision','HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\IrisVision','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\IrisVision')) { $keys[$key] = Registry-Snapshot $key }
    $links = [ordered]@{}
    foreach ($path in @((Join-Path $desktop 'IrisVision.lnk'),(Join-Path $programs 'IrisVision.lnk'))) { $links[$path] = if (Test-Path -LiteralPath $path) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash } else { $null } }
    return [ordered]@{registry=$keys; shortcut_sha256=$links}
}
function Replace-Define([string]$Text,[string]$Name,[string]$Value) {
    if ($Value -match '["$\r\n]') { throw "Unsafe NSIS define value for $Name" }
    $pattern = '(?m)^!define ' + [regex]::Escape($Name) + ' "[^"\r\n]*"\r?$'
    if ([regex]::Matches($Text,$pattern).Count -ne 1) { throw "Expected exactly one NSIS definition: $Name" }
    return [regex]::Replace($Text,$pattern,('!define ' + $Name + ' "' + $Value + '"'))
}
function Invoke-OwnedProcess([string]$Executable,[string]$Arguments,[string]$Label,[int]$TimeoutSeconds=120) {
    [void](Assert-QaPath $Executable)
    $stdout = Join-Path $qaRoot ($Label + '.stdout.log')
    $stderr = Join-Path $qaRoot ($Label + '.stderr.log')
    $process = Start-Process -FilePath $Executable -ArgumentList $Arguments -WorkingDirectory $qaRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        $report.process_timeout=$Label
        # NSIS may start a temporary uninstaller outside installRoot. Kill the
        # owned process tree rather than relying on a filename/path scan.
        $process.Kill($true)
        if (-not $process.WaitForExit(10000)) { throw "Owned QA process tree did not terminate: $Label" }
        throw "Owned QA process exceeded deadline: $Label"
    }
    $process.Refresh()
    if ($process.ExitCode -ne 0) { throw "Owned QA process failed ($($process.ExitCode)): $Label" }
    return [ordered]@{label=$Label; exit_code=$process.ExitCode; stdout=$stdout; stderr=$stderr}
}
function Assert-NoQaProcesses {
    $matches = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($installRoot + '\',[StringComparison]::OrdinalIgnoreCase) })
    if ($matches.Count -ne 0) { throw 'QA installed processes are still running; refusing installer/upgrade/uninstall' }
}
function Verify-Installed([string]$Version,[string]$Label) {
    $registration = Get-ItemProperty -LiteralPath $uninstallKey
    if ($registration.DisplayName -ne $product -or $registration.DisplayVersion -ne $Version -or $registration.Publisher -ne $publisher -or $registration.MainBinaryName -ne ($binaryName + '.exe') -or $registration.InstallLocation.Trim('"') -ine $installRoot) { throw 'QA uninstall registration does not match the unique namespace' }
    if ((Get-Content -LiteralPath (Join-Path $installRoot 'QA-VERSION.txt') -Raw).Trim() -ne $Version) { throw 'QA resource replacement marker mismatch' }
    if ((Get-FileHash -LiteralPath (Join-Path $installRoot ($binaryName + '.exe')) -Algorithm SHA256).Hash -ne $sourceShellHash) { throw 'Installed main payload differs from the original staged shell' }
    foreach ($resource in $resourceMap) {
        $installed = Assert-QaPath (Join-Path $installRoot $resource.destination)
        if (-not (Test-Path -LiteralPath $installed -PathType Leaf) -or (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -ne $resource.sha256) { throw "Installed resource mismatch: $($resource.destination)" }
    }
    $shortcutTargets = [ordered]@{}
    $wsh = New-Object -ComObject WScript.Shell
    foreach ($shortcut in $qaShortcuts) {
        if (-not (Test-Path -LiteralPath $shortcut -PathType Leaf)) { throw "QA shortcut missing: $shortcut" }
        $target = $wsh.CreateShortcut($shortcut).TargetPath
        if ($target -ine (Join-Path $installRoot ($binaryName + '.exe'))) { throw 'QA shortcut target escaped installed QA binary' }
        $shortcutTargets[$shortcut]=$target
    }
    $hostReport = Join-Path $qaRoot ($Label + '-host.json')
    $savedEnvironment = @{}
    foreach ($name in @('IRIS_DATA_DIR','IRIS_DAEMON_PATH','IRIS_MODEL_DIR','IRIS_DAEMON_WORKER_PATH')) {
        $savedEnvironment[$name]=[Environment]::GetEnvironmentVariable($name,'Process')
        Remove-Item -LiteralPath ('Env:' + $name) -ErrorAction SilentlyContinue
    }
    try {
        $env:IRIS_DATA_DIR = $dataRoot
        $hostStep = Invoke-OwnedProcess (Join-Path $installRoot ($binaryName + '.exe')) ('--verify-host "' + $hostReport + '"') ($Label + '-host') 60
    } finally {
        foreach ($name in $savedEnvironment.Keys) {
            if ($null -eq $savedEnvironment[$name]) { Remove-Item -LiteralPath ('Env:' + $name) -ErrorAction SilentlyContinue }
            else { [Environment]::SetEnvironmentVariable($name,$savedEnvironment[$name],'Process') }
        }
    }
    $hostResult = Get-Content -LiteralPath $hostReport -Raw | ConvertFrom-Json
    if (-not $hostResult.ok -or -not $hostResult.heartbeat) { throw 'Installed headless host check failed' }
    $rawReport = $null
    if ($VerifyRawFallback) {
        $rawOutput = Join-Path $qaRoot ($Label + '-raw-fallback')
        & python (Join-Path $repositoryRoot 'tools/verify-raw-fallback.py') --daemon (Join-Path $installRoot 'iris-daemon.exe') --models (Join-Path $installRoot 'models') --output $rawOutput *> (Join-Path $qaRoot ($Label + '-raw-fallback.log'))
        if ($LASTEXITCODE -ne 0) { throw 'Installed RAW fallback verification failed' }
        $rawReport = Join-Path $rawOutput 'report.json'
        $rawResult = Get-Content -LiteralPath $rawReport -Raw | ConvertFrom-Json
        if (-not $rawResult.ok -or -not $rawResult.source_hashes_preserved -or $rawResult.exiftool_present) { throw 'Installed RAW fallback report failed its assertions' }
    }
    Assert-NoQaProcesses
    return [ordered]@{version=$Version; payload_sha256=$sourceShellHash; resource_files_verified=$resourceMap.Count; shortcut_targets=$shortcutTargets; host=$hostStep; host_payload_version=$hostResult.version; raw_fallback_report=$rawReport}
}

if (Test-Path -LiteralPath $qaRoot) { throw 'QA root must be new' }
foreach ($path in @($uninstallKey,$publisherKey)+$qaShortcuts) { if (Test-Path -LiteralPath $path) { throw "QA namespace already exists: $path" } }
if ($qaRoot -match '["$\r\n]') { throw 'QA root cannot be represented safely in NSIS defines' }
$realBefore = Real-State
$sourceHashes = [ordered]@{}
foreach ($path in @($sourceNsi,$sourceUtils,$sourceShell,$sourceInstaller)) { $sourceHashes[$path]=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
$sourceShellHash = $sourceHashes[$sourceShell]
$originalNsi = Get-Content -LiteralPath $sourceNsi -Raw
$utils = Get-Content -LiteralPath $sourceUtils -Raw
if ($originalNsi -notmatch '!define INSTALLMODE "currentUser"' -or $originalNsi -notmatch '!define INSTALLWEBVIEW2MODE ""') { throw 'Only the current-user, no-runtime-installer package is eligible' }
if ($utils -notmatch 'RestartManager_RegisterFile \$R0 "\$\{executablePath\}"' -or $utils -match '(?i)(FindProcess|KillProcess|taskkill|nsProcess)') { throw 'Unexpected NSIS process lookup/termination implementation; needs review' }
if ($originalNsi -match '(?im)^\s*!macro\s+NSIS_HOOK_' -or $originalNsi -match '(?im)^\s*!insertmacro\s+(APP_ASSOCIATE|APP_UNASSOCIATE)') { throw 'Unexpected custom hook or file association; needs review' }
$bundleConfig = Get-Content -LiteralPath (Join-Path $sourceReport.stage 'bundle.json') -Raw | ConvertFrom-Json
$resourceMap = @($bundleConfig.bundle.resources.PSObject.Properties | ForEach-Object { [pscustomobject]@{source=$_.Name;destination=[string]$_.Value;sha256=(Get-FileHash -LiteralPath $_.Name -Algorithm SHA256).Hash} })
if ($VerifyRawFallback) {
    foreach ($required in @('iris-raw-decoder.exe','sources/raw-decoder-source.zip')) {
        if ($required -notin $resourceMap.destination) { throw "RAW lifecycle check requires installed resource: $required" }
    }
}
New-Item -ItemType Directory -Path $qaRoot,$dataRoot,$photosRoot,$sourceRoot | Out-Null
Copy-Item -LiteralPath $sourceShell -Destination (Join-Path $sourceRoot ($binaryName + '.exe'))
$installed = $false
$failure = $null
try {
    $installers = @()
    foreach ($version in @('0.1.0','0.1.1')) {
        $buildRoot = Join-Path $qaRoot ('build-' + $version)
        New-Item -ItemType Directory -Path $buildRoot | Out-Null
        Get-ChildItem -LiteralPath (Split-Path -Parent $sourceNsi) -Filter '*.nsh' -File | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $buildRoot }
        $marker = Join-Path $buildRoot 'QA-VERSION.txt'
        [IO.File]::WriteAllText($marker,$version,[Text.UTF8Encoding]::new($false))
        $output = Join-Path $buildRoot 'qa-installer.exe'
        $text = $originalNsi
        foreach ($entry in ([ordered]@{PRODUCTNAME=$product; MANUFACTURER=$publisher; BUNDLEID=$bundleId; MAINBINARYNAME=$binaryName; MAINBINARYSRCPATH=(Join-Path $sourceRoot ($binaryName + '.exe')); VERSION=$version; VERSIONWITHBUILD=($version+'.0'); OUTFILE=$output}).GetEnumerator()) { $text = Replace-Define $text $entry.Key $entry.Value }
        $fileNeedle = '  File "${MAINBINARYSRCPATH}"'
        $deleteNeedle = '  Delete "$INSTDIR\${MAINBINARYNAME}.exe"'
        if ([regex]::Matches($text,[regex]::Escape($fileNeedle)).Count -ne 1 -or [regex]::Matches($text,[regex]::Escape($deleteNeedle)).Count -ne 1) { throw 'Unexpected installer resource anchors' }
        $text = $text.Replace($fileNeedle, $fileNeedle + "`r`n" + '  File /oname=QA-VERSION.txt "' + $marker + '"')
        $text = $text.Replace($deleteNeedle, $deleteNeedle + "`r`n" + '  Delete "$INSTDIR\QA-VERSION.txt"')
        if ($text -match '(?m)^!define (PRODUCTNAME "IrisVision"|MANUFACTURER "irisvision"|BUNDLEID "local\.irisvision\.app"|MAINBINARYNAME "iris-shell")') { throw 'Production identity remains in QA installer definitions' }
        $script = Join-Path $buildRoot 'installer.nsi'
        [IO.File]::WriteAllText($script,$text,[Text.UTF8Encoding]::new($false))
        $audit = [ordered]@{product=$product;publisher=$publisher;bundle_id=$bundleId;main_binary=$binaryName;install_mode='currentUser';process_scope='RestartManager exact QA executable path';uninstall_registry=$uninstallKey;product_registry=$productKey;shortcuts=$qaShortcuts;script_sha256=(Get-FileHash -LiteralPath $script -Algorithm SHA256).Hash;source_payload_sha256=$sourceShellHash}
        $audit | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $buildRoot 'scope-audit.json') -Encoding utf8
        Push-Location $buildRoot
        try {
            & $compiler /V2 /INPUTCHARSET UTF8 $script *> (Join-Path $buildRoot 'compile.log')
            if ($LASTEXITCODE -ne 0) { throw "QA NSIS compile failed: $version" }
        } finally { Pop-Location }
        if (-not (Test-Path -LiteralPath $output -PathType Leaf)) { throw 'QA installer was not produced' }
        $installers += $output
    }
    $report.scope_audited=$true
    $report.real_state_before=$realBefore
    Assert-NoQaProcesses
    # NSIS requires /D to be the final, unquoted argument, including paths with spaces.
    $report.steps += Invoke-OwnedProcess $installers[0] ('/S /D=' + $installRoot) 'install-v1'
    $installed=$true
    $report.installed_v1 = Verify-Installed '0.1.0' 'v1'
    $sentinel = Join-Path $dataRoot 'qa-user-data.txt'
    [IO.File]::WriteAllText($sentinel,('data survives '+$nonce),[Text.UTF8Encoding]::new($false))
    $sentinelHash = (Get-FileHash -LiteralPath $sentinel -Algorithm SHA256).Hash
    $database = Join-Path $dataRoot 'library.sqlite3'
    $databaseHash = (Get-FileHash -LiteralPath $database -Algorithm SHA256).Hash
    Assert-NoQaProcesses
    # /UPDATE matches the installed updater's NSIS mode. Deliberately omit /R.
    $report.steps += Invoke-OwnedProcess $installers[1] ('/S /UPDATE /D=' + $installRoot) 'upgrade-v2'
    if ((Get-FileHash -LiteralPath $sentinel -Algorithm SHA256).Hash -ne $sentinelHash -or (Get-FileHash -LiteralPath $database -Algorithm SHA256).Hash -ne $databaseHash) { throw 'QA user data changed during installer upgrade' }
    $report.data_preserved_across_upgrade=$true
    $report.installed_v2 = Verify-Installed '0.1.1' 'v2'
    $databaseAfterHostHash = (Get-FileHash -LiteralPath $database -Algorithm SHA256).Hash
    Assert-NoQaProcesses
    # Run a copy outside INSTDIR so the uninstaller can remove its installed copy.
    $uninstaller = Join-Path $qaRoot 'qa-uninstall.exe'
    Copy-Item -LiteralPath (Join-Path $installRoot 'uninstall.exe') -Destination $uninstaller
    $report.steps += Invoke-OwnedProcess $uninstaller ('/S _?=' + $installRoot) 'uninstall-v2'
    if (Test-Path -LiteralPath $uninstallKey) { throw 'QA uninstall registration survived actual uninstall' }
    foreach ($path in $qaShortcuts) { if (Test-Path -LiteralPath $path) { throw 'QA shortcut survived actual uninstall' } }
    if (Test-Path -LiteralPath $installRoot) { throw 'QA installation directory survived actual uninstall' }
    if ((Get-FileHash -LiteralPath $sentinel -Algorithm SHA256).Hash -ne $sentinelHash -or (Get-FileHash -LiteralPath $database -Algorithm SHA256).Hash -ne $databaseAfterHostHash) { throw 'QA user data changed during uninstall' }
    $report.installed_files_removed=$true
    $report.uninstall_registration_removed=$true
    $report.qa_shortcuts_removed=$true
    $report.data_preserved_across_uninstall=$true
    # NSIS deliberately preserves remembered location/language without the GUI's
    # delete-data checkbox. Report that behavior, then remove only our random key.
    $report.remembered_location_after_uninstall=Registry-Snapshot $productKey
    $report.ok=$true
} catch {
    $failure=$_
    $report.error=$_.Exception.Message
} finally {
    try {
        if (Test-Path -LiteralPath (Join-Path $installRoot 'uninstall.exe')) {
            Assert-NoQaProcesses
            $cleanupUninstaller=Join-Path $qaRoot 'qa-cleanup-uninstall.exe'
            Copy-Item -LiteralPath (Join-Path $installRoot 'uninstall.exe') -Destination $cleanupUninstaller -Force
            [void](Invoke-OwnedProcess $cleanupUninstaller ('/S _?=' + $installRoot) 'cleanup-uninstall')
        }
        # Every key/name was checked absent before creation and contains our nonce.
        foreach ($key in @($uninstallKey,$publisherKey)) {
            if (-not $key.EndsWith($nonce,[StringComparison]::Ordinal)) { throw 'Refusing cleanup of a non-QA registry key' }
            if (Test-Path -LiteralPath $key) { Remove-Item -LiteralPath $key -Recurse -Force }
        }
        foreach ($shortcut in $qaShortcuts) {
            if ([IO.Path]::GetFileNameWithoutExtension($shortcut) -ne $product) { throw 'Refusing cleanup of a non-QA shortcut' }
            if (Test-Path -LiteralPath $shortcut) { Remove-Item -LiteralPath $shortcut -Force }
        }
        if (Test-Path -LiteralPath $installRoot) {
            $resolvedInstall=Assert-QaPath (Resolve-Path -LiteralPath $installRoot).Path
            Remove-Item -LiteralPath $resolvedInstall -Recurse -Force
        }
        $realAfter=Real-State
        $report.real_state_after=$realAfter
        if (($realBefore | ConvertTo-Json -Depth 20 -Compress) -cne ($realAfter | ConvertTo-Json -Depth 20 -Compress)) { throw 'Original product registry/shortcuts changed during QA' }
        foreach ($path in $sourceHashes.Keys) { if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $sourceHashes[$path]) { throw 'Original delivery artifact changed during QA' } }
        $report.real_product_state_unchanged=$true
        $report.source_hashes_unchanged=$sourceHashes
        # A timeout needs separate review of possible NSIS temporary descendants;
        # do not claim full cleanup merely because our known paths are now absent.
        $report.cleanup_complete=-not $report.Contains('process_timeout')
    } catch {
        $report.ok=$false
        $report.cleanup_error=$_.Exception.Message
        if (-not $failure) { $failure=$_ }
    }
    $report | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $reportPath -Encoding utf8
}
if ($failure) { throw "QA lifecycle verification failed: $($failure.Exception.Message). Report: $reportPath" }
Write-Output ('Verified isolated NSIS install/metadata upgrade/uninstall: '+$reportPath)
