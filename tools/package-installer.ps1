param(
    [Parameter(Mandatory=$true)][string]$PortableDirectory,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [switch]$EnableUpdater,
    [string]$UpdateEndpoint,
    [string]$UpdatePublicKeyFile,
    [ValidatePattern('^[0-9A-Fa-f]{40}$')][string]$CertificateThumbprint,
    [string]$TimestampUrl,
    [switch]$PrepareOnly
)
$ErrorActionPreference = 'Stop'
function Assert-ExpectedSignature([string]$Path, [string]$Thumbprint) {
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or $null -eq $signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ine $Thumbprint) {
        throw "Authenticode signature does not match the expected valid certificate: $Path"
    }
}
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$portableRoot = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $PortableDirectory).ProviderPath)
$outputRoot = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $outputRoot) { throw 'Output directory must be new' }
$publicKey = $null
if ($EnableUpdater) {
    $endpointUri = $null
    if (-not [Uri]::TryCreate($UpdateEndpoint, [UriKind]::Absolute, [ref]$endpointUri) -or $endpointUri.Scheme -ne 'https' -or $endpointUri.UserInfo -or $endpointUri.Fragment) { throw 'Updater requires a trusted HTTPS endpoint without credentials or fragment' }
    if (-not $UpdatePublicKeyFile) { throw 'Updater requires an explicit public key file' }
    $publicKey = (Get-Content -LiteralPath $UpdatePublicKeyFile -Raw).Trim()
    if (-not $publicKey) { throw 'Updater public key is empty' }
    if (-not $env:TAURI_SIGNING_PRIVATE_KEY) { throw 'Updater artifacts require TAURI_SIGNING_PRIVATE_KEY; no keys are generated automatically' }
} elseif ($UpdateEndpoint -or $UpdatePublicKeyFile) { throw 'Update settings require -EnableUpdater' }
$signTool = $null
if ($CertificateThumbprint) {
    $timestampUri = $null
    if (-not [Uri]::TryCreate($TimestampUrl,[UriKind]::Absolute,[ref]$timestampUri) -or $timestampUri.Scheme -notin @('https','http') -or $timestampUri.UserInfo) { throw 'Authenticode signing requires an explicit timestamp URL' }
    $signTool = (Get-Command signtool.exe -ErrorAction Stop).Source
} elseif ($TimestampUrl) { throw 'Timestamp URL requires a signing certificate' }
$manifestPath = Join-Path $portableRoot 'checksums.json'
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.configuration -ne 'Release') { throw 'Installer requires a verified Release portable bundle' }
$version = (Get-Content -LiteralPath (Join-Path $repositoryRoot 'apps/shell/package.json') -Raw | ConvertFrom-Json).version
if ($manifest.version -ne $version) { throw 'Portable version does not match the application version' }
$validated = @{}
foreach ($entry in $manifest.files) {
    $relative = $entry.path.Replace('\','/')
    if ([System.IO.Path]::IsPathRooted($relative) -or $relative.Split('/') -contains '..') { throw "Invalid manifest path: $relative" }
    $path = [System.IO.Path]::GetFullPath((Join-Path $portableRoot $relative))
    if (-not $path.StartsWith($portableRoot.TrimEnd('\','/') + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) { throw 'Manifest path escaped package' }
    if ($validated.ContainsKey($relative)) { throw "Duplicate manifest path: $relative" }
    $file = Get-Item -LiteralPath $path
    if ($file.PSIsContainer -or ($file.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) { throw "Expected regular file: $relative" }
    if ($file.Length -ne $entry.bytes -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $entry.sha256) { throw "Portable checksum mismatch: $relative" }
    $validated[$relative] = $file.FullName
}
foreach ($required in @('iris-shell.exe','iris-daemon.exe','iris-cli.exe','iris-mcp.exe','iris-raw-decoder.exe','sources/raw-decoder-source.zip','sources/iris-application-source.zip','LICENSE','THIRD_PARTY_NOTICES.md','WebView2Loader.dll','models/manifest.json','models/onnxruntime.dll')) {
    if (-not $validated.ContainsKey($required)) { throw "Missing required package file: $required" }
}
if (-not $PrepareOnly) {
    if (-not $validated.ContainsKey('licenses/rust-dependencies.json')) { throw 'Portable bundle lacks its dependency license index' }
    $licenseIndex = Get-Content -LiteralPath $validated['licenses/rust-dependencies.json'] -Raw | ConvertFrom-Json
    $licensed = @{}
    foreach ($entry in $licenseIndex) { $licensed[$entry.name + '-' + $entry.version] = $entry }
    $nativeMetadataText = & cargo metadata --manifest-path (Join-Path $repositoryRoot 'apps/shell/src-tauri/Cargo.toml') --locked --features desktop --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect native dependency licenses' }
    $nativeMetadata = $nativeMetadataText | ConvertFrom-Json
    foreach ($package in $nativeMetadata.packages) {
        if (-not $package.source) { continue }
        $key = $package.name + '-' + $package.version
        if (-not $licensed.ContainsKey($key)) { throw "Portable licenses lack $key; rebuild the portable bundle from current sources first" }
        foreach ($name in $licensed[$key].files) {
            if (-not $validated.ContainsKey("licenses/rust/$key/$name")) { throw "Missing licensed dependency file: $key/$name" }
        }
    }
}
# Isolate the bundler's in-place executable patching from tested release binaries.
$stage = Join-Path $repositoryRoot ('artifacts/installer-' + [guid]::NewGuid().ToString('N'))
$target = Join-Path $stage 'target'
$release = Join-Path $target 'release'
$payload = Join-Path $stage 'payload'
New-Item -ItemType Directory -Path $release,$payload | Out-Null
Copy-Item -LiteralPath $validated['iris-shell.exe'] -Destination (Join-Path $release 'iris-shell.exe')
$resources = [ordered]@{}
foreach ($relative in ($validated.Keys | Sort-Object)) {
    # Carry the exact approved application and RAW source archives and notices.
    # Do not recursively copy arbitrary source or development directories.
    if ($relative -notin @('iris-daemon.exe','iris-cli.exe','iris-mcp.exe','iris-raw-decoder.exe','sources/raw-decoder-source.zip','sources/iris-application-source.zip','LICENSE','THIRD_PARTY_NOTICES.md','WebView2Loader.dll','openapi.json') -and -not $relative.StartsWith('models/') -and -not $relative.StartsWith('licenses/')) { continue }
    if ($relative.StartsWith('models/optional/',[StringComparison]::OrdinalIgnoreCase)) { throw 'Optional model files are not permitted in the installer' }
    $destination = Join-Path $payload $relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
    Copy-Item -LiteralPath $validated[$relative] -Destination $destination
    $resources[$destination] = $relative
}
$installationNotes = Join-Path $payload 'INSTALLATION.txt'
@"
Iris $version - Windows x64 NSIS build
Installs for the current Windows user with the desktop selection interface.
Microsoft Edge WebView2 Runtime and Microsoft Visual C++ 2015-2022 x64
Redistributable must already be installed. Default models work offline.
RAW fallback uses the independent iris-raw-decoder.exe component. Its source
archive is included at sources/raw-decoder-source.zip; see licenses for terms.
Iris is GPL-3.0-or-later. Application source: sources/iris-application-source.zip.
See LICENSE and THIRD_PARTY_NOTICES.md. Use iris-mcp.exe for headless stdio MCP.
CLI usage: iris-cli.exe --database <path> --model-dir <installed-folder>\models --help
No photos, user databases, private keys, or optional research weights are included.
Updater configured: $([bool]$EnableUpdater). Installing an update is a separate explicit command.
Building this installer does not establish clean-Windows or real-upgrade validation.
"@ | Set-Content -LiteralPath $installationNotes -Encoding utf8
$resources[$installationNotes] = 'INSTALLATION.txt'
# The same transparent build resource used by the hidden shell; no product artwork.
$iconPath = Join-Path $stage 'host.ico'
$iconBytes = [System.Collections.Generic.List[byte]]::new()
$iconBytes.AddRange([byte[]]@(0,0,1,0,1,0,1,1,0,0,1,0,32,0,48,0,0,0,22,0,0,0))
foreach ($value in @([uint32]40,[uint32]1,[uint32]2)) { $iconBytes.AddRange([BitConverter]::GetBytes($value)) }
foreach ($value in @([uint16]1,[uint16]32)) { $iconBytes.AddRange([BitConverter]::GetBytes($value)) }
$iconBytes.AddRange([byte[]]::new(28))
$iconBytes.AddRange([byte[]]@(255,255,255,255))
[System.IO.File]::WriteAllBytes($iconPath, $iconBytes.ToArray())
$config = [ordered]@{
    bundle = [ordered]@{
        active = $true
        targets = @('nsis')
        createUpdaterArtifacts = [bool]$EnableUpdater
        icon = @($iconPath)
        resources = $resources
        windows = [ordered]@{
            webviewInstallMode = @{type='skip'}
            nsis = @{installMode='currentUser'; languages=@('English','SimpChinese'); displayLanguageSelector=$false; installerIcon=$iconPath; uninstallerIcon=$iconPath}
        }
    }
}
if ($CertificateThumbprint) {
    $config.bundle.windows.certificateThumbprint = $CertificateThumbprint
    $config.bundle.windows.timestampUrl = $TimestampUrl
    $config.bundle.windows.tsp = $true
}
if ($EnableUpdater) { $config.plugins = @{updater=@{pubkey=$publicKey; requireSignedVersion=$true}} }
$configPath = Join-Path $stage 'bundle.json'
$config | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $configPath -Encoding utf8
$sourceShellHash = (Get-FileHash -LiteralPath $validated['iris-shell.exe'] -Algorithm SHA256).Hash.ToLowerInvariant()
if ($PrepareOnly) {
    New-Item -ItemType Directory -Path $outputRoot | Out-Null
    @{prepared=$true; stage=$stage; config=$configPath; resource_count=$resources.Count; source_shell_sha256=$sourceShellHash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outputRoot 'preparation.json') -Encoding utf8
    return
}
$previousTarget = $env:CARGO_TARGET_DIR
$buildEnvironment = @('IRIS_DISTRIBUTION','IRIS_UPDATE_ENDPOINT','IRIS_UPDATE_PUBLIC_KEY')
$previousBuildEnvironment = @{}
foreach ($name in $buildEnvironment) { $previousBuildEnvironment[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
Push-Location (Join-Path $repositoryRoot 'apps/shell')
try {
    # All installers identify as NSIS, even when no update channel is configured.
    # The dedicated profile keeps this and embedded trust out of portable binaries.
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    $env:IRIS_DISTRIBUTION = 'nsis'
    if ($EnableUpdater) {
        $env:IRIS_UPDATE_ENDPOINT = $UpdateEndpoint
        $env:IRIS_UPDATE_PUBLIC_KEY = $publicKey
    } else {
        Remove-Item Env:IRIS_UPDATE_ENDPOINT -ErrorAction SilentlyContinue
        Remove-Item Env:IRIS_UPDATE_PUBLIC_KEY -ErrorAction SilentlyContinue
    }
    & npm run build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed' }
    & cargo build --manifest-path src-tauri/Cargo.toml --locked --features desktop --profile release-installer
    if ($LASTEXITCODE -ne 0) { throw 'NSIS native build failed' }
    Copy-Item -LiteralPath 'src-tauri/target/release-installer/iris-shell.exe' -Destination (Join-Path $release 'iris-shell.exe') -Force
    if ($CertificateThumbprint) {
        # These executables are packaged resources, not Tauri externalBin entries.
        # Sign their staged copies explicitly; never mutate the input portable.
        foreach ($binaryName in @('iris-daemon.exe','iris-cli.exe','iris-raw-decoder.exe')) {
            $binaryPath = Join-Path $payload $binaryName
            & $signTool sign /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 $binaryPath
            if ($LASTEXITCODE -ne 0) { throw "Authenticode signing failed for $binaryName" }
            Assert-ExpectedSignature -Path $binaryPath -Thumbprint $CertificateThumbprint
        }
    }
    $env:CARGO_TARGET_DIR = $target
    $bundleArgs = @('exec','--yes','--package=@tauri-apps/cli@2.12.1','--','tauri','bundle','--features','desktop','--bundles','nsis','--config',$configPath,'--ci')
    # Tauri --no-sign suppresses updater signatures as well as Authenticode.
    if (-not $CertificateThumbprint -and -not $EnableUpdater) { $bundleArgs += '--no-sign' }
    & npm @bundleArgs
    if ($LASTEXITCODE -ne 0) { throw 'Tauri NSIS bundle failed' }
} finally {
    if ($null -eq $previousTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue } else { $env:CARGO_TARGET_DIR = $previousTarget }
    foreach ($name in $buildEnvironment) {
        if ($null -eq $previousBuildEnvironment[$name]) {
            Remove-Item -LiteralPath ("Env:" + $name) -ErrorAction SilentlyContinue
        } else {
            [Environment]::SetEnvironmentVariable($name,$previousBuildEnvironment[$name],'Process')
        }
    }
    Pop-Location
}
$installers = @(Get-ChildItem -LiteralPath (Join-Path $release 'bundle/nsis') -Filter '*-setup.exe' -File)
if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer' }
if ((Get-FileHash -LiteralPath $validated['iris-shell.exe'] -Algorithm SHA256).Hash -ine $sourceShellHash) { throw 'Source portable shell changed during bundling' }
if ($EnableUpdater) {
    $signature = $installers[0].FullName + '.sig'
    if (-not (Test-Path -LiteralPath $signature -PathType Leaf)) { throw 'Updater signature missing; do not publish this installer' }
    & cargo run --manifest-path (Join-Path $repositoryRoot 'apps/shell/src-tauri/Cargo.toml') --locked --features updater-client --example verify_update_artifact -- $installers[0].FullName $signature (Resolve-Path -LiteralPath $UpdatePublicKeyFile).Path $version
    if ($LASTEXITCODE -ne 0) { throw 'Updater artifact signature/public key/version verification failed; output was not published' }
}
if ($CertificateThumbprint) {
    Assert-ExpectedSignature -Path $installers[0].FullName -Thumbprint $CertificateThumbprint
    Assert-ExpectedSignature -Path (Join-Path $release 'iris-shell.exe') -Thumbprint $CertificateThumbprint
}
# Publish locally only after all requested verification has succeeded in staging.
New-Item -ItemType Directory -Path $outputRoot | Out-Null
$installer = Join-Path $outputRoot $installers[0].Name
Copy-Item -LiteralPath $installers[0].FullName -Destination $installer
if ($EnableUpdater) { Copy-Item -LiteralPath $signature -Destination ($installer + '.sig') }
$report = [ordered]@{
    version=$version; installer=[System.IO.Path]::GetFileName($installer)
    installer_sha256=(Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    installer_bytes=(Get-Item -LiteralPath $installer).Length
    source_portable=$portableRoot; source_manifest_sha256=(Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
    source_shell_sha256=$sourceShellHash; bundled_shell_sha256=(Get-FileHash -LiteralPath (Join-Path $release 'iris-shell.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    stage=$stage; cli_version='2.12.1'; bundle='nsis'; install_mode='currentUser'
    authenticode_status=(Get-AuthenticodeSignature -LiteralPath $installer).Status.ToString()
    updater_configured=[bool]$EnableUpdater; updater_signature_created=[bool]$EnableUpdater
    resources=$resources.Count; source_checksums_verified=$true; original_shell_preserved=$true
    installer_executed=$false; clean_windows_verified=$false
    prerequisites=@('Microsoft Edge WebView2 Runtime','Microsoft Visual C++ 2015-2022 x64 Redistributable')
}
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputRoot 'build-report.json') -Encoding utf8
Write-Output "NSIS installer: $installer"
