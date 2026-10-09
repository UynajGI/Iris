$ErrorActionPreference = 'Stop'
$repository = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$script = Join-Path $repository 'tools/package-local.ps1'
$parseErrors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($script, [ref]$null, [ref]$parseErrors)
if ($parseErrors) { throw 'Packaging script has syntax errors' }
$copyFunction = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Copy-DefaultModels' }, $true)
if (-not $copyFunction) { throw 'Default-model copy function missing' }
. ([scriptblock]::Create($copyFunction.Extent.Text))
$temporary = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$testRoot = Join-Path $temporary ('iris-model-copy-' + [guid]::NewGuid().ToString('N'))
try {
    $source = Join-Path $testRoot 'source'
    $destination = Join-Path $testRoot 'bundle-models'
    New-Item -ItemType Directory -Path (Join-Path $source 'optional/nested') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $source 'artifacts/faceocc-research') -Force | Out-Null
    foreach ($file in @('README.md','onnxruntime.dll','yunet.onnx','niqe_params.json','LICENSE-YuNet.txt')) { Set-Content -LiteralPath (Join-Path $source $file) -Value ('default-' + $file) }
    Set-Content -LiteralPath (Join-Path $source 'optional/nested/restricted.onnx') -Value 'DO NOT DISTRIBUTE'
    Set-Content -LiteralPath (Join-Path $source 'unlisted-private.onnx') -Value 'DO NOT DISTRIBUTE'
    Set-Content -LiteralPath (Join-Path $source 'optional/faceocc.onnx') -Value 'RESEARCH ONLY - DO NOT DISTRIBUTE'
    Set-Content -LiteralPath (Join-Path $source 'faceocc.onnx') -Value 'UNLISTED RESEARCH MODEL'
    Set-Content -LiteralPath (Join-Path $source 'artifacts/faceocc-research/faceocc.onnx') -Value 'RESEARCH ONLY - DO NOT DISTRIBUTE'
    $mediaRoot = Join-Path $source 'media'
    $mediaFiles = @('libheif.dll','libde265.dll','libwinpthread-1.dll','README.txt','licenses/libheif-COPYING.txt','licenses/libde265-COPYING.txt','licenses/libwinpthread/COPYING','licenses/winpthreads/COPYING','licenses/gcc-libs/COPYING.LIB','licenses/gcc-libs/COPYING.RUNTIME','licenses/gcc-libs/COPYING3','licenses/gcc-libs/README','sources/setup-heif-runtime.py','sources/libheif-v1.23.6.tar.gz','sources/libde265-v1.1.3.tar.gz')
    $entries = @($mediaFiles | ForEach-Object {
        $path = Join-Path $mediaRoot $_
        New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
        Set-Content -LiteralPath $path -Value ('fixture-' + $_)
        @{path=$_;size=(Get-Item -LiteralPath $path).Length;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    $mediaManifest = @{
        schema=1;platform='windows-x64';hevc_decoder='libde265';plugins=$false;files=$entries
        sources=@(@{name='libheif';version='1.23.6';sha256=($entries | Where-Object path -eq 'sources/libheif-v1.23.6.tar.gz').sha256},@{name='libde265';version='1.1.3';sha256=($entries | Where-Object path -eq 'sources/libde265-v1.1.3.tar.gz').sha256})
        dependencies=@{'libheif.dll'=@('KERNEL32.dll','msvcrt.dll','libde265.dll','libwinpthread-1.dll');'libde265.dll'=@('KERNEL32.dll','msvcrt.dll','libwinpthread-1.dll');'libwinpthread-1.dll'=@('KERNEL32.dll','msvcrt.dll')}
    } | ConvertTo-Json -Depth 8
    $mediaManifestPath = Join-Path $mediaRoot 'manifest.json'
    Set-Content -LiteralPath $mediaManifestPath -Value $mediaManifest
    @{models=@(@{file='yunet.onnx'});niqe=@{file='niqe_params.json'}} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $source 'manifest.json')
    Copy-DefaultModels -SourceDirectory $source -DestinationDirectory $destination
    if (Test-Path -LiteralPath (Join-Path $destination 'optional')) { throw 'Optional directory was copied' }
    if (Test-Path -LiteralPath (Join-Path $destination 'unlisted-private.onnx')) { throw 'Unlisted model was copied' }
    if ((Test-Path -LiteralPath (Join-Path $destination 'faceocc.onnx')) -or (Test-Path -LiteralPath (Join-Path $destination 'artifacts'))) { throw 'Research FaceOcc weights were copied' }
    if (-not (Test-Path -LiteralPath (Join-Path $destination 'yunet.onnx'))) { throw 'Default model missing' }
    if ((Get-ChildItem -LiteralPath $destination -Recurse -File).Count -ne 22) { throw 'Unexpected copied model files' }
    foreach ($entry in $entries) {
        $copied = Join-Path $destination ('media/' + $entry.path)
        if ((Get-FileHash -LiteralPath $copied -Algorithm SHA256).Hash -ine $entry.sha256) { throw 'Media closure was not copied exactly' }
    }
    function Assert-CopyRejected([string]$Label) {
        $target = Join-Path $testRoot ('rejected-' + $Label)
        $rejected = $false
        try { Copy-DefaultModels -SourceDirectory $source -DestinationDirectory $target } catch { $rejected = $true }
        if (-not $rejected -or (Test-Path -LiteralPath $target)) { throw "Invalid input copied files before rejection: $Label" }
    }
    foreach ($case in @('traversal','duplicate','hash','missing-license','missing-source','source-hash','dependency')) {
        $invalid = $mediaManifest | ConvertFrom-Json
        switch ($case) {
            'traversal' { $invalid.files[0].path = '../optional/faceocc.onnx' }
            'duplicate' { $invalid.files += $invalid.files[0] }
            'hash' { $invalid.files[0].sha256 = '0' * 64 }
            'missing-license' { $invalid.files = @($invalid.files | Where-Object path -ne 'licenses/libheif-COPYING.txt') }
            'missing-source' { $invalid.files = @($invalid.files | Where-Object path -ne 'sources/libheif-v1.23.6.tar.gz') }
            'source-hash' { $invalid.sources[0].sha256 = '0' * 64 }
            'dependency' { $invalid.dependencies.'libheif.dll' += 'missing.dll' }
        }
        $invalid | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $mediaManifestPath
        Assert-CopyRejected $case
    }
    Set-Content -LiteralPath $mediaManifestPath -Value $mediaManifest
    $privateMedia = Join-Path $mediaRoot 'private.onnx'
    Set-Content -LiteralPath $privateMedia -Value 'DO NOT DISTRIBUTE'
    Assert-CopyRejected 'unlisted-media'
    Remove-Item -LiteralPath $privateMedia
    $runtimePath = Join-Path $mediaRoot 'libheif.dll'
    $runtimeBytes = [IO.File]::ReadAllBytes($runtimePath)
    [IO.File]::WriteAllBytes($runtimePath, [byte[]]@(0))
    Assert-CopyRejected 'corrupt-runtime'
    [IO.File]::WriteAllBytes($runtimePath, $runtimeBytes)
    @{models=@(@{file='scrfd_500m.onnx'})} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $source 'manifest.json')
    Assert-CopyRejected 'scrfd'
    @{models=@(@{file='optional/nested/restricted.onnx'})} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $source 'manifest.json')
    $rejected = $false
    try { Copy-DefaultModels -SourceDirectory $source -DestinationDirectory (Join-Path $testRoot 'rejected') } catch { $rejected = $true }
    if (-not $rejected) { throw 'Manifest was allowed to include optional weights' }
    if (Test-Path -LiteralPath (Join-Path $testRoot 'rejected')) { throw 'Invalid manifest copied files before rejection' }
    Write-Output 'Default-model copy passed: complete media hashes/dependencies/licenses/sources; malformed closure rejected before copying; SCRFD, optional and unlisted weights excluded'
} finally {
    $resolved = [System.IO.Path]::GetFullPath($testRoot)
    if (-not $resolved.StartsWith($temporary, [StringComparison]::OrdinalIgnoreCase) -or $resolved -eq $temporary) { throw 'Unsafe temporary cleanup path' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
