$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

$root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-package-test-$([guid]::NewGuid())"
$input = Join-Path $root "input"
$output = Join-Path $root "output"
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
} finally {
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
