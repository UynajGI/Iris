param(
    [ValidateSet('Debug','Release')][string]$Configuration = 'Release',
    [string]$OutputDirectory,
    [switch]$SkipBuild,
    [switch]$SkipVerify,
    [switch]$IncludeDirectML
)
function Copy-DefaultModels {
    param([string]$SourceDirectory, [string]$DestinationDirectory)
    $sourceRoot = (Resolve-Path -LiteralPath $SourceDirectory).Path
    $sourceManifest = Get-Content -LiteralPath (Join-Path $sourceRoot 'manifest.json') -Raw | ConvertFrom-Json
    foreach ($model in $sourceManifest.models) {
        if ($model.file -notin @('yunet.onnx','face_landmarks_detector.onnx','face_blendshapes.onnx')) { throw "Model is not approved for the general bundle: $($model.file)" }
    }
    $defaultFiles = @('manifest.json', 'README.md', 'onnxruntime.dll')
    $defaultFiles += @($sourceManifest.models | ForEach-Object { $_.file })
    if ($sourceManifest.niqe.file) { $defaultFiles += $sourceManifest.niqe.file }
    $defaultFiles += @(Get-ChildItem -LiteralPath $sourceRoot -File | Where-Object { $_.Name -like 'LICENSE-*.txt' -or $_.Name -like 'ThirdPartyNotices-*.txt' } | ForEach-Object { $_.Name })
    $copyPlan = @()
    foreach ($name in ($defaultFiles | Sort-Object -Unique)) {
        # Only explicit top-level default files may enter a general-purpose bundle.
        # Never recurse into optional/, including via a manifest entry or link.
        if (-not $name -or [System.IO.Path]::GetFileName($name) -ne $name -or $name -match '[/\\]' -or $name -ieq 'optional') { throw "Invalid default model filename: $name" }
        $file = Get-Item -LiteralPath (Join-Path $sourceRoot $name)
        if ($file.PSIsContainer -or ($file.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) { throw "Default model entry must be a regular file: $name" }
        $copyPlan += $file.FullName
    }
    # HEIC's runtime, license notices and corresponding sources form one explicit
    # closure. Validate everything before creating a destination or copying bytes.
    $mediaRoot = Join-Path $sourceRoot 'media'
    $mediaDirectory = Get-Item -LiteralPath $mediaRoot
    if (-not $mediaDirectory.PSIsContainer -or ($mediaDirectory.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) { throw 'Media runtime must be a regular directory' }
    $mediaManifestPath = Join-Path $mediaRoot 'manifest.json'
    $mediaManifestFile = Get-Item -LiteralPath $mediaManifestPath
    if ($mediaManifestFile.PSIsContainer -or ($mediaManifestFile.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) { throw 'Media manifest must be a regular file' }
    $mediaManifest = Get-Content -LiteralPath $mediaManifestPath -Raw | ConvertFrom-Json
    if ($mediaManifest.schema -ne 1 -or $mediaManifest.platform -ne 'windows-x64' -or $mediaManifest.hevc_decoder -ne 'libde265' -or $mediaManifest.plugins -ne $false) { throw 'Unsupported media runtime manifest' }
    $mediaPlan = @{}
    foreach ($entry in $mediaManifest.files) {
        $relative = [string]$entry.path
        if (-not $relative -or $relative -match '[\\:]' -or [IO.Path]::IsPathRooted($relative) -or @($relative.Split('/') | Where-Object { $_ -in @('', '.', '..') }).Count) { throw "Invalid media manifest path: $relative" }
        if ($relative -notin @('libheif.dll','libde265.dll','libwinpthread-1.dll','README.txt') -and -not $relative.StartsWith('licenses/') -and -not $relative.StartsWith('sources/')) { throw "Unapproved media runtime file: $relative" }
        if ($relative -match '(?i)\.(onnx|exe)$') { throw "Models/executables cannot enter the media closure: $relative" }
        if ($mediaPlan.ContainsKey($relative)) { throw "Duplicate media manifest path: $relative" }
        $path = $mediaRoot
        foreach ($component in $relative.Split('/')) {
            $path = Join-Path $path $component
            $item = Get-Item -LiteralPath $path
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked media entries are not allowed: $relative" }
        }
        if ($item.PSIsContainer -or $entry.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or $item.Length -ne $entry.size -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $entry.sha256) { throw "Media runtime checksum mismatch: $relative" }
        $mediaPlan[$relative] = $item.FullName
    }
    foreach ($required in @('libheif.dll','libde265.dll','libwinpthread-1.dll','README.txt','licenses/libheif-COPYING.txt','licenses/libde265-COPYING.txt','licenses/libwinpthread/COPYING','licenses/winpthreads/COPYING','licenses/gcc-libs/COPYING.LIB','licenses/gcc-libs/COPYING.RUNTIME','licenses/gcc-libs/COPYING3','licenses/gcc-libs/README','sources/setup-heif-runtime.py')) {
        if (-not $mediaPlan.ContainsKey($required)) { throw "Media closure is missing a required file: $required" }
    }
    $sourceNames = @{}
    foreach ($source in $mediaManifest.sources) {
        if ($source.name -notin @('libheif','libde265') -or $sourceNames.ContainsKey($source.name) -or $source.version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid media source declaration' }
        $sourceNames[$source.name] = $true
        $relative = 'sources/' + $source.name + '-v' + $source.version + '.tar.gz'
        if (-not $mediaPlan.ContainsKey($relative) -or (Get-FileHash -LiteralPath $mediaPlan[$relative] -Algorithm SHA256).Hash -ine $source.sha256) { throw "Media corresponding source is missing or corrupt: $relative" }
    }
    if ($sourceNames.Count -ne 2) { throw 'Media runtime requires both corresponding source archives' }
    $dlls = @('libheif.dll','libde265.dll','libwinpthread-1.dll')
    if (@($mediaManifest.dependencies.PSObject.Properties).Count -ne $dlls.Count) { throw 'Media dependency graph is incomplete' }
    foreach ($dll in $dlls) {
        $dependencies = $mediaManifest.dependencies.PSObject.Properties[$dll]
        if (-not $dependencies -or @($dependencies.Value).Count -eq 0) { throw "Media dependency graph lacks $dll" }
        foreach ($dependency in $dependencies.Value) {
            if ($dependency -notin @('KERNEL32.dll','msvcrt.dll') -and $dependency -notin $dlls) { throw "Unresolved media dependency: $dll -> $dependency" }
        }
    }
    foreach ($item in (Get-ChildItem -LiteralPath $mediaRoot -Recurse -Force)) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked entries are not allowed anywhere in media runtime' }
        if (-not $item.PSIsContainer) {
            $relative = [IO.Path]::GetRelativePath($mediaRoot,$item.FullName).Replace('\','/')
            if ($relative -ne 'manifest.json' -and -not $mediaPlan.ContainsKey($relative)) { throw "Unlisted media runtime file: $relative" }
        }
    }
    New-Item -ItemType Directory -Path $DestinationDirectory | Out-Null
    foreach ($file in $copyPlan) { Copy-Item -LiteralPath $file -Destination $DestinationDirectory }
    $mediaDestination = Join-Path $DestinationDirectory 'media'
    New-Item -ItemType Directory -Path $mediaDestination | Out-Null
    Copy-Item -LiteralPath $mediaManifestPath -Destination $mediaDestination
    foreach ($relative in ($mediaPlan.Keys | Sort-Object)) {
        $target = Join-Path $mediaDestination $relative
        New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
        Copy-Item -LiteralPath $mediaPlan[$relative] -Destination $target
        $entry = $mediaManifest.files | Where-Object { $_.path -ceq $relative }
        if ((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ine $entry.sha256) { throw "Copied media checksum mismatch: $relative" }
    }
    if (Test-Path -LiteralPath (Join-Path $DestinationDirectory 'optional')) { throw 'Optional model directory must never be packaged' }
}
$ErrorActionPreference = 'Stop'
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$frontendRoot = Join-Path $repositoryRoot 'apps/shell'
$version = (Get-Content -LiteralPath (Join-Path $frontendRoot 'package.json') -Raw | ConvertFrom-Json).version
$profile = $Configuration.ToLowerInvariant()
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repositoryRoot "dist/portable/Iris-$version-windows-x64-$profile" }
$outputRoot = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $outputRoot) { throw "Output already exists; choose a new directory: $outputRoot" }
Push-Location $repositoryRoot
try {
    if (-not $SkipBuild) {
        & npm --prefix $frontendRoot ci --no-audit --no-fund
        if ($LASTEXITCODE -ne 0) { throw 'Frontend dependency installation failed' }
        & npm --prefix $frontendRoot run build
        if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed' }
        $buildFlags = @('--locked')
        if ($Configuration -eq 'Release') { $buildFlags += '--release' }
        & cargo build -p iris-daemon -p iris-cli -p iris-mcp @buildFlags
        if ($LASTEXITCODE -ne 0) { throw 'Daemon build failed' }
        & cargo build --manifest-path (Join-Path $repositoryRoot 'components/raw-decoder/Cargo.toml') @buildFlags
        if ($LASTEXITCODE -ne 0) { throw 'Standalone RAW converter build failed' }
        & cargo build --manifest-path (Join-Path $frontendRoot 'src-tauri/Cargo.toml') --features desktop @buildFlags
        if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed' }
    }
    $shellExe = Join-Path $frontendRoot "src-tauri/target/$profile/iris-shell.exe"
    $daemonExe = Join-Path $repositoryRoot "target/$profile/iris-daemon.exe"
    $cliExe = Join-Path $repositoryRoot "target/$profile/iris-cli.exe"
    $mcpExe = Join-Path $repositoryRoot "target/$profile/iris-mcp.exe"
    $rawExe = Join-Path $repositoryRoot "components/raw-decoder/target/$profile/iris-raw-decoder.exe"
    $webviewLoader = Join-Path $frontendRoot "src-tauri/target/$profile/WebView2Loader.dll"
    foreach ($file in @($shellExe,$daemonExe,$cliExe,$mcpExe,$rawExe,$webviewLoader,(Join-Path $frontendRoot 'dist/index.html'))) { if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing build artifact: $file" } }
    New-Item -ItemType Directory -Path $outputRoot | Out-Null
    Copy-Item -LiteralPath $shellExe,$daemonExe,$cliExe,$mcpExe,$rawExe,$webviewLoader -Destination $outputRoot
    # Runtime JavaScript/assets only. TypeScript declarations, source maps and
    # development files are provided separately in the application source archive.
    $webRoot = Join-Path $frontendRoot 'dist'
    foreach ($file in (Get-ChildItem -LiteralPath $webRoot -Recurse -File)) {
        if ($file.Extension -notin @('.html','.js','.css','.json','.png','.svg','.ico','.woff','.woff2','.ttf','.txt')) { continue }
        $relative = [IO.Path]::GetRelativePath($webRoot, $file.FullName)
        $destination = Join-Path (Join-Path $outputRoot 'frontend') $relative
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $destination
    }
    Copy-Item -LiteralPath (Join-Path $frontendRoot 'portable/Launch.ps1'),(Join-Path $frontendRoot 'portable/Verify.ps1') -Destination $outputRoot
    Copy-DefaultModels -SourceDirectory (Join-Path $repositoryRoot 'models') -DestinationDirectory (Join-Path $outputRoot 'models')
    & python (Join-Path $PSScriptRoot 'package-runtime-closure.py') --kind raw --source (Join-Path $repositoryRoot 'models/raw') --destination (Join-Path $outputRoot 'models/raw')
    if ($LASTEXITCODE -ne 0) { throw 'RAW runtime closure verification failed' }
    if ($IncludeDirectML) {
        & python (Join-Path $PSScriptRoot 'package-runtime-closure.py') --kind directml --source (Join-Path $repositoryRoot 'models/directml') --destination (Join-Path $outputRoot 'models/directml')
        if ($LASTEXITCODE -ne 0) { throw 'DirectML runtime closure verification failed' }
    }
    & python (Join-Path $PSScriptRoot 'package-relink-source.py') --output (Join-Path $outputRoot 'sources/raw-decoder-source.zip')
    if ($LASTEXITCODE -ne 0) { throw 'LGPL relink source packaging failed' }
    & python (Join-Path $PSScriptRoot 'package-source.py') --output (Join-Path $outputRoot 'sources/iris-application-source.zip')
    if ($LASTEXITCODE -ne 0) { throw 'Application source export failed; commit and validate the release tree first' }
    & python (Join-Path $PSScriptRoot 'package-dependency-source.py') --output (Join-Path $outputRoot 'sources/dependency-sources.zip')
    if ($LASTEXITCODE -ne 0) { throw 'Dependency source packaging failed' }
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'LICENSE'),(Join-Path $repositoryRoot 'THIRD_PARTY_NOTICES.md') -Destination $outputRoot
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'docs/openapi.json') -Destination (Join-Path $outputRoot 'openapi.json')
    $licenses = Join-Path $outputRoot 'licenses'
    New-Item -ItemType Directory -Path $licenses | Out-Null
    Copy-Item -LiteralPath (Join-Path $frontendRoot 'node_modules/react/LICENSE') -Destination (Join-Path $licenses 'React-LICENSE.txt')
    Get-ChildItem -LiteralPath (Join-Path $frontendRoot 'portable/licenses') -File | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $licenses }
    # Rust bindings omit Microsoft's SDK license files; retrieve the exact SDK
    # matching the distributed loader and verify the DLL bytes before using them.
    $sdkVersion = (Get-Item -LiteralPath $webviewLoader).VersionInfo.FileVersion.Trim()
    $sdkUrl = "https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/$sdkVersion/microsoft.web.webview2.$sdkVersion.nupkg"
    $http = New-Object System.Net.Http.HttpClient
    $sdkBytes = $http.GetByteArrayAsync($sdkUrl).GetAwaiter().GetResult()
    $sdkStream = New-Object System.IO.MemoryStream(,$sdkBytes)
    $sdkZip = New-Object System.IO.Compression.ZipArchive($sdkStream)
    $sdkDllStream = $sdkZip.GetEntry('build/native/x64/WebView2Loader.dll').Open()
    $hasher = [System.Security.Cryptography.SHA256]::Create()
    $sdkDllHash = [Convert]::ToHexString($hasher.ComputeHash($sdkDllStream)).ToLowerInvariant()
    $sdkDllStream.Dispose()
    if ($sdkDllHash -ne (Get-FileHash -LiteralPath $webviewLoader -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'NuGet SDK loader differs from compiled loader' }
    foreach ($sdkLicense in @('LICENSE.txt','NOTICE.txt')) {
        $reader = New-Object System.IO.StreamReader($sdkZip.GetEntry($sdkLicense).Open())
        $reader.ReadToEnd() | Set-Content -LiteralPath (Join-Path $licenses ('Microsoft-WebView2-SDK-' + $sdkLicense)) -Encoding utf8
        $reader.Dispose()
    }
    [ordered]@{version=$sdkVersion;source=$sdkUrl;archive_sha256=[Convert]::ToHexString($hasher.ComputeHash($sdkBytes)).ToLowerInvariant();loader_sha256=$sdkDllHash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $licenses 'Microsoft-WebView2-SDK-provenance.json') -Encoding utf8
    $hasher.Dispose(); $sdkZip.Dispose(); $sdkStream.Dispose(); $http.Dispose()
    $licenseIndex = @{}
    foreach ($manifest in @((Join-Path $repositoryRoot 'Cargo.toml'),(Join-Path $frontendRoot 'src-tauri/Cargo.toml'),(Join-Path $repositoryRoot 'components/raw-decoder/Cargo.toml'))) {
        $metadataArguments = @('metadata','--manifest-path',$manifest,'--locked','--format-version','1')
        # The shell's dependencies are optional and enabled by desktop builds.
        # Default-feature metadata alone omits the actual native dependency tree.
        if ($manifest -eq (Join-Path $frontendRoot 'src-tauri/Cargo.toml')) { $metadataArguments += @('--features','desktop') }
        $metadataText = & cargo @metadataArguments
        if ($LASTEXITCODE -ne 0) { throw 'Cannot enumerate Rust dependency licenses' }
        $metadata = $metadataText | ConvertFrom-Json
        foreach ($package in $metadata.packages) {
            if (-not $package.source) { continue }
            $key = $package.name + '-' + $package.version
            if ($licenseIndex.ContainsKey($key)) { continue }
            $packageRoot = Split-Path -Parent $package.manifest_path
            $packageLicenses = @(Get-ChildItem -LiteralPath $packageRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)' })
            if ($package.license_file) {
                $specifiedLicense = Join-Path $packageRoot $package.license_file
                if (Test-Path -LiteralPath $specifiedLicense -PathType Leaf) { $packageLicenses += Get-Item -LiteralPath $specifiedLicense }
            }
            $destination = Join-Path $licenses ('rust/' + $key)
            New-Item -ItemType Directory -Path $destination -Force | Out-Null
            foreach ($licenseFile in $packageLicenses) { Copy-Item -LiteralPath $licenseFile.FullName -Destination $destination -Force }
            $licenseIndex[$key] = [ordered]@{name=$package.name;version=$package.version;license=$package.license;files=@($packageLicenses.Name | Sort-Object -Unique)}
        }
    }
    @($licenseIndex.Keys | Sort-Object | ForEach-Object { $licenseIndex[$_] }) | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $licenses 'rust-dependencies.json') -Encoding utf8
    @"
Iris $version - unsigned local $Configuration portable build

Run Launch.ps1 from PowerShell. Model/runtime paths resolve from this folder.
The desktop interface requires Windows x64 and Microsoft Edge WebView2 Runtime.
Microsoft Visual C++ 2015-2022 x64 Redistributable is required by ONNX Runtime.
Install the official Microsoft redistributable if missing: https://aka.ms/vs/17/release/vc_redist.x64.exe
Run Verify.ps1 to check SHA-256 files, outside-repository startup and synthetic JPG inference.
Headless usage (PowerShell, from this folder):
  `$modelDir = (Resolve-Path '.\models').Path
  .\iris-cli.exe --database 'C:\IrisData\library.sqlite3' --model-dir `$modelDir scan 'C:\Photos'
  .\iris-cli.exe --database 'C:\IrisData\library.sqlite3' --model-dir `$modelDir analyze <project-id>
Use --help for all commands. Keep iris-cli.exe beside iris-daemon.exe for isolated workers.
For manual daemon startup, --worker-limit 1..16 controls each project's analysis pool;
available CPUs and pending photos also limit it. It is not a global process or memory cap.
No source photographs, user settings, database, or credentials are included.
Model licenses/notices are in models; dependency licenses are in licenses.
Offline HEIC decoding includes the models/media DLL closure, licenses and corresponding sources, all manifest-verified.
RAW previews include standalone ExifTool under models/raw and iris-raw-decoder.exe fallback.
sources/raw-decoder-source.zip contains only that standalone converter and its dependencies, with offline rebuild instructions for LGPL rawler.
The main application is GPL-3.0-or-later; see LICENSE and THIRD_PARTY_NOTICES.md.
sources/iris-application-source.zip contains the committed application sources and build scripts.
Third-party dependency/source and model compatibility review remains required before public binary release.
The MCP executable uses stdio without starting the desktop; run iris-mcp.exe --help.
DirectML runtime included: $IncludeDirectML. CPU remains the default; select execution_provider=directml explicitly.
Only default distributable model files are included. User-supplied models/optional weights are excluded before copying.
An optional DINOv3 offline bundle is distributed separately; it is never silently installed or enabled by this package.
Optional detector weights are never downloaded or licensed on the user's behalf.
This local bundle is unsigned. Signing and remote update publication are not configured.
"@ | Set-Content -LiteralPath (Join-Path $outputRoot 'README.txt') -Encoding utf8
    if (Test-Path -LiteralPath (Join-Path $outputRoot 'models/optional')) { throw 'Optional model directory must never be packaged' }
    $entries = @(Get-ChildItem -LiteralPath $outputRoot -Recurse -File | Sort-Object FullName | ForEach-Object {
        [ordered]@{path=[System.IO.Path]::GetRelativePath($outputRoot,$_.FullName).Replace('\','/');bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    [ordered]@{product='Iris';version=$version;configuration=$Configuration;files=$entries} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputRoot 'checksums.json') -Encoding utf8
    if (-not $SkipVerify) { & (Join-Path $outputRoot 'Verify.ps1'); if (-not $?) { throw 'Portable verification failed' } }
    Write-Output ('Portable bundle: ' + $outputRoot)
} finally { Pop-Location }
