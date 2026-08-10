$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Assert-Fails([scriptblock]$Action, [string]$Message) {
    try {
        & $Action
    } catch {
        return
    }
    throw $Message
}

$root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-package-test-$([guid]::NewGuid())"
$input = Join-Path $root "input"
$output = Join-Path $root "output"
$unsignedOutput = Join-Path $root "unsigned"
[IO.Directory]::CreateDirectory($input) | Out-Null
$installer = Join-Path $input "setup.exe"
$signature = Join-Path $input "setup.exe.sig"
[IO.File]::WriteAllBytes($installer, [Text.Encoding]::UTF8.GetBytes("installer fixture"))
[IO.File]::WriteAllText($signature, "trusted-signature")

try {
    $result = & (Join-Path $PSScriptRoot "package-windows.ps1") `
        -Installer $installer `
        -UpdaterSignature $signature `
        -OutputDirectory $output `
        -Version "2.0.0-alpha.4" `
        -ReleaseBaseUrl "https://downloads.example.test/copypaste" `
        -PublishDate "2026-08-09T00:00:00Z" | ConvertFrom-Json
    $name = "CopyPaste-v2.0.0-alpha.4-windows-x86_64-setup.exe"
    Assert-True ((Split-Path -Leaf $result.artifact) -eq $name) "artifact name is not canonical"
    Assert-True ((Get-FileHash -LiteralPath $result.artifact -Algorithm SHA256).Hash.ToLowerInvariant() -eq $result.sha256) "reported hash differs"

    $latest = Get-Content -Raw -LiteralPath (Join-Path $output "latest.json") | ConvertFrom-Json
    $platform = $latest.platforms.'windows-x86_64'
    Assert-True ($platform.signature -eq "trusted-signature") "updater signature was not copied into metadata"
    Assert-True ($platform.url -eq "https://downloads.example.test/copypaste/$name") "updater URL is not canonical"
    $checksums = Get-Content -LiteralPath (Join-Path $output "SHA256SUMS")
    Assert-True ($checksums.Count -eq 3) "signed release must checksum all three artifacts"
    Assert-True (-not ($checksums -match [regex]::Escape($root))) "checksums disclose a local path"

    $unsigned = & (Join-Path $PSScriptRoot "package-windows.ps1") `
        -Installer $installer `
        -OutputDirectory $unsignedOutput `
        -Version "2.0.0-alpha.4" | ConvertFrom-Json
    Assert-True (Test-Path -LiteralPath $unsigned.artifact) "unsigned artifact is missing"
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $unsignedOutput "latest.json"))) "unsigned package must not publish update metadata"
    Assert-True (@(Get-Content -LiteralPath (Join-Path $unsignedOutput "SHA256SUMS")).Count -eq 1) "unsigned release must checksum only its installer"

    Assert-Fails {
        & (Join-Path $PSScriptRoot "package-windows.ps1") `
            -Installer $installer `
            -UpdaterSignature $signature `
            -OutputDirectory (Join-Path $root "broken") `
            -Version "2.0.0-alpha.4"
    } "an updater signature without its release URL must fail"
} finally {
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
