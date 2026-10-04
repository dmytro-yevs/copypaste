param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+$')]
    [string]$Version,
    [ValidateRange(1, 2147483647)]
    [int]$BuildNumber = 1,
    [string]$OutputDirectory = "dist"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($env:OS -ne "Windows_NT") { throw "Windows release packaging requires Windows" }
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$appRoot = Join-Path $root "apps\copypaste_flutter"
$cargoToml = Get-Content -LiteralPath (Join-Path $root "Cargo.toml") -Raw
$pubspec = Get-Content -LiteralPath (Join-Path $appRoot "pubspec.yaml") -Raw
$cargoMatch = [regex]::Match(
    $cargoToml,
    '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^\"]+)"'
)
$pubspecMatch = [regex]::Match($pubspec, '(?m)^version:\s*([^\s]+)\s*$')
if (-not $cargoMatch.Success -or $cargoMatch.Groups[1].Value -cne $Version) {
    throw "Cargo.toml does not identify version $Version"
}
if (-not $pubspecMatch.Success -or $pubspecMatch.Groups[1].Value -cne "$Version+$BuildNumber") {
    throw "pubspec.yaml does not identify version $Version+$BuildNumber"
}

Push-Location $root
try {
    & cargo build --release --locked -p copypaste-daemon -p copypaste-cli
    if ($LASTEXITCODE -ne 0) { throw "Rust release build failed" }
    Push-Location $appRoot
    try {
        & flutter pub get --enforce-lockfile
        if ($LASTEXITCODE -ne 0) { throw "Flutter dependency resolution failed" }
        & flutter build windows --release --build-name $Version --build-number $BuildNumber
        if ($LASTEXITCODE -ne 0) { throw "Flutter Windows release build failed" }
    } finally {
        Pop-Location
    }

    $releaseDirectory = Join-Path $appRoot "build\windows\x64\runner\Release"
    if (-not (Test-Path -LiteralPath (Join-Path $releaseDirectory "CopyPaste.exe") -PathType Leaf)) {
        throw "Flutter did not produce CopyPaste.exe"
    }
    Copy-Item -LiteralPath (Join-Path $root "target\release\copypaste-daemon.exe") `
        -Destination (Join-Path $releaseDirectory "copypaste-daemon.exe") -Force
    Copy-Item -LiteralPath (Join-Path $root "target\release\copypaste.exe") `
        -Destination (Join-Path $releaseDirectory "copypaste.exe") -Force

    & (Join-Path $PSScriptRoot "windows-sign.ps1") -Operation Validate
    if ($LASTEXITCODE -ne 0) { throw "Windows signing state is invalid" }
    Get-ChildItem -LiteralPath $releaseDirectory -Recurse -File |
        Where-Object { $_.Extension -in @('.exe', '.dll') } |
        ForEach-Object {
            & (Join-Path $PSScriptRoot "windows-sign.ps1") -Operation Sign -File $_.FullName
            & (Join-Path $PSScriptRoot "windows-sign.ps1") -Operation Verify -File $_.FullName
        }

    $makensis = (Get-Command makensis.exe -ErrorAction Stop).Source
    $outputRoot = [IO.Path]::GetFullPath((Join-Path $root $OutputDirectory))
    New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
    $installer = Join-Path $outputRoot "CopyPaste-v$Version-windows-x86_64-setup.exe"
    & $makensis "/DVERSION=$Version" "/DSOURCE_DIR=$releaseDirectory" `
        "/DOUTPUT_FILE=$installer" (Join-Path $root "packaging\windows\copypaste.nsi")
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $installer -PathType Leaf)) {
        throw "NSIS did not produce the Windows installer"
    }
    & (Join-Path $PSScriptRoot "windows-sign.ps1") -Operation Sign -File $installer
    & (Join-Path $PSScriptRoot "windows-sign.ps1") -Operation Verify -File $installer

    $digest = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$installer.sha256", "$digest  $([IO.Path]::GetFileName($installer))`n")
    Write-Host "Built $([IO.Path]::GetFileName($installer))"
} finally {
    Pop-Location
}
