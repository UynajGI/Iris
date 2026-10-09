param(
    [Parameter(Mandatory=$true)][string]$PortableDirectory,
    [Parameter(Mandatory=$true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$outputRoot = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $outputRoot) { throw 'Verification output directory must be new' }
$keyRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('iris-signing-proof-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $keyRoot | Out-Null
$keyPath = Join-Path $keyRoot 'test.key'
$publicKeyPath = $keyPath + '.pub'
$previousPrivateKey = $env:TAURI_SIGNING_PRIVATE_KEY
$previousPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
try {
    # Ephemeral test trust only; never print or preserve the generated private key.
    & npm exec --yes --package=@tauri-apps/cli@2.12.1 -- tauri signer generate --ci --password 'ephemeral-local-test' --write-keys $keyPath | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Cannot generate temporary test signing material' }
    $env:TAURI_SIGNING_PRIVATE_KEY = $keyPath
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = 'ephemeral-local-test'
    & (Join-Path $PSScriptRoot 'package-installer.ps1') -PortableDirectory $PortableDirectory -OutputDirectory $outputRoot -EnableUpdater -UpdateEndpoint 'https://updates.invalid/iris.json' -UpdatePublicKeyFile $publicKeyPath
    if (-not $?) { throw 'Signed test installer build failed' }
    $build = Get-Content -LiteralPath (Join-Path $outputRoot 'build-report.json') -Raw | ConvertFrom-Json
    $artifact = Join-Path $outputRoot $build.installer
    Copy-Item -LiteralPath $publicKeyPath -Destination (Join-Path $outputRoot 'test-public-key.txt')
    # A one-byte change must fail the same verifier, without ever running the setup.
    $corrupted = Join-Path $keyRoot 'corrupted-setup.exe'
    $bytes = [System.IO.File]::ReadAllBytes($artifact)
    $bytes[$bytes.Length - 1] = $bytes[$bytes.Length - 1] -bxor 1
    [System.IO.File]::WriteAllBytes($corrupted,$bytes)
    & cargo run --manifest-path (Join-Path $repositoryRoot 'apps/shell/src-tauri/Cargo.toml') --locked --features updater-client --example verify_update_artifact -- $corrupted ($artifact + '.sig') $publicKeyPath $build.version *> (Join-Path $outputRoot 'tamper-rejection.log')
    if ($LASTEXITCODE -eq 0) { throw 'Signature verifier accepted modified installer bytes' }
    if ((Get-Content -LiteralPath (Join-Path $outputRoot 'tamper-rejection.log') -Raw) -notmatch 'Artifact signature or public key is invalid') { throw 'Tamper rejection failed for an unrelated tool/runtime reason' }
    & cargo run --manifest-path (Join-Path $repositoryRoot 'apps/shell/src-tauri/Cargo.toml') --locked --features updater-client --example verify_update_artifact -- $artifact ($artifact + '.sig') $publicKeyPath '999.0.0' *> (Join-Path $outputRoot 'version-rejection.log')
    if ($LASTEXITCODE -eq 0) { throw 'Signature verifier accepted different release version' }
    if ((Get-Content -LiteralPath (Join-Path $outputRoot 'version-rejection.log') -Raw) -notmatch 'Signature must contain exactly the expected release version') { throw 'Version rejection failed for an unrelated tool/runtime reason' }
    [ordered]@{ok=$true; test_trust_only=$true; endpoint='https://updates.invalid/iris.json'; valid_signature_verified=$true; tampered_bytes_rejected=$true; different_version_rejected=$true; installer_executed=$false; public_key='test-public-key.txt'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outputRoot 'signature-verification.json') -Encoding utf8
} finally {
    if ($null -eq $previousPrivateKey) { Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue } else { $env:TAURI_SIGNING_PRIVATE_KEY = $previousPrivateKey }
    if ($null -eq $previousPassword) { Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue } else { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $previousPassword }
    # Remove only the exact files created by this invocation; never recurse.
    foreach ($ownedFile in @($keyPath,$publicKeyPath,(Join-Path $keyRoot 'corrupted-setup.exe'))) {
        if (Test-Path -LiteralPath $ownedFile -PathType Leaf) { Remove-Item -LiteralPath $ownedFile -Force }
    }
    if (Test-Path -LiteralPath $keyRoot -PathType Container) { Remove-Item -LiteralPath $keyRoot }
}
Write-Output "Updater signing proof (test trust only, never installed): $outputRoot"
