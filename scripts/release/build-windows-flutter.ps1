param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+$')]
    [string]$Version,
    [ValidateRange(1, 2147483647)]
    [int]$BuildNumber = 1,
    [string]$OutputDirectory = "dist",
    # All builds and signs locally; external signing runs Application, Installer, and Finalize
    # separately and signs the staged files between stages.
    [ValidateSet("All", "Application", "Installer", "Finalize")]
    [string]$Stage = "All",
    [ValidateSet("Local", "External")]
    [string]$Signing = "Local",
    [string]$SigningDirectory = "target\windows-signing"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($env:OS -ne "Windows_NT") { throw "Windows release packaging requires Windows" }
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$appRoot = Join-Path $root "apps\copypaste_flutter"
& python (Join-Path $root "scripts/release/version.py") check --version $Version --build-number $BuildNumber
if ($LASTEXITCODE -ne 0) { throw "Release identity does not match the requested Windows build" }

if ($Stage -eq "All" -and $Signing -ne "Local") { throw "A single-stage build requires local signing" }
$releaseDirectory = Join-Path $appRoot "build\windows\x64\runner\Release"
$signingRoot = [IO.Path]::GetFullPath((Join-Path $root $SigningDirectory))
$outputRoot = [IO.Path]::GetFullPath((Join-Path $root $OutputDirectory))
$installerName = "CopyPaste-v$Version-windows-x86_64-setup.exe"
$installer = Join-Path $outputRoot $installerName
$signScript = Join-Path $PSScriptRoot "windows-sign.ps1"
# External signing covers only binaries built from this repository; upstream libraries stay unsigned.
$projectBinaries = @(
    "CopyPaste.exe", "copypaste-cli.exe", "copypaste-daemon.exe",
    "copypaste-update-helper.exe", "copypaste_flutter_bridge.dll"
)

function Get-ApplicationBinaries {
    return @(Get-ChildItem -LiteralPath $releaseDirectory -Recurse -File |
        Where-Object { $_.Extension -in @('.exe', '.dll') } |
        Sort-Object FullName -CaseSensitive)
}

function Get-ProjectBinaries {
    $binaries = Get-ApplicationBinaries
    return @($projectBinaries | ForEach-Object {
        $name = $_
        $matching = @($binaries | Where-Object { $_.Name -ceq $name })
        if ($matching.Count -ne 1) { throw "Expected exactly one $name in the Windows application" }
        $matching[0]
    })
}

function Import-SignedFiles([string]$SignedDirectory, [hashtable]$Destinations) {
    if (-not (Test-Path -LiteralPath $SignedDirectory -PathType Container)) {
        throw "Externally signed files are missing"
    }
    $signed = @(Get-ChildItem -LiteralPath $SignedDirectory -Recurse -File |
        ForEach-Object { [IO.Path]::GetRelativePath($SignedDirectory, $_.FullName) } | Sort-Object -CaseSensitive)
    $expected = @($Destinations.Keys | Sort-Object -CaseSensitive)
    if ([string]::Join("`n", $signed) -cne [string]::Join("`n", $expected)) {
        throw "Externally signed files do not match the submitted files"
    }
    foreach ($name in $expected) {
        Copy-Item -LiteralPath (Join-Path $SignedDirectory $name) -Destination $Destinations[$name] -Force
    }
}

function Build-Application {
    & python (Join-Path $root "scripts/ci/timed.py") compilation cargo build --release --locked -p copypaste-daemon -p copypaste-cli
    if ($LASTEXITCODE -ne 0) { throw "Rust release build failed" }
    Push-Location $appRoot
    try {
        & python (Join-Path $root "scripts/ci/timed.py") dependencies flutter pub get --enforce-lockfile
        if ($LASTEXITCODE -ne 0) { throw "Flutter dependency resolution failed" }
        & python (Join-Path $root "scripts/ci/timed.py") compilation flutter build windows --release --no-pub --build-name $Version --build-number $BuildNumber
        if ($LASTEXITCODE -ne 0) { throw "Flutter Windows release build failed" }
    } finally {
        Pop-Location
    }

    if (-not (Test-Path -LiteralPath (Join-Path $releaseDirectory "CopyPaste.exe") -PathType Leaf)) {
        throw "Flutter did not produce CopyPaste.exe"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $releaseDirectory "copypaste-update-helper.exe") -PathType Leaf)) {
        throw "Flutter did not produce the standalone update helper"
    }
    $applicationHash = (Get-FileHash -LiteralPath (Join-Path $releaseDirectory "CopyPaste.exe") -Algorithm SHA256).Hash
    Copy-Item -LiteralPath (Join-Path $root "target\release\copypaste-daemon.exe") `
        -Destination (Join-Path $releaseDirectory "copypaste-daemon.exe") -Force
    Copy-Item -LiteralPath (Join-Path $root "target\release\copypaste.exe") `
        -Destination (Join-Path $releaseDirectory "copypaste-cli.exe") -Force
    if ((Get-FileHash -LiteralPath (Join-Path $releaseDirectory "CopyPaste.exe") -Algorithm SHA256).Hash -cne $applicationHash) {
        throw "Bundling runtime helpers changed the Flutter application"
    }

    & $signScript -Operation Validate
    if ($LASTEXITCODE -ne 0) { throw "Windows signing state is invalid" }
    if ($Signing -eq "Local") {
        # Signing asserts the same signer, chain, and timestamp policy as verification.
        Get-ApplicationBinaries | ForEach-Object { & $signScript -Operation Sign -File $_.FullName }
    } else {
        Remove-Item -LiteralPath $signingRoot -Recurse -Force -ErrorAction SilentlyContinue
        $staging = Join-Path $signingRoot "unsigned-application"
        New-Item -ItemType Directory -Force -Path $staging | Out-Null
        Get-ProjectBinaries | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $staging }
    }
}

function Build-Installer {
    if ($Signing -eq "Local") {
        Get-ApplicationBinaries | ForEach-Object { & $signScript -Operation Verify -File $_.FullName }
    } else {
        $staging = Join-Path $signingRoot "unsigned-application"
        $destinations = @{}
        foreach ($binary in Get-ProjectBinaries) {
            $submitted = Join-Path $staging $binary.Name
            if ((Get-FileHash -LiteralPath $binary.FullName -Algorithm SHA256).Hash -cne
                (Get-FileHash -LiteralPath $submitted -Algorithm SHA256).Hash) {
                throw "$($binary.Name) changed after submission for signing"
            }
            $destinations[$binary.Name] = $binary.FullName
        }
        Import-SignedFiles (Join-Path $signingRoot "signed-application") $destinations
        Get-ProjectBinaries | ForEach-Object { & $signScript -Operation Verify -File $_.FullName }
    }

    $makensis = (Get-Command makensis.exe -ErrorAction Stop).Source
    $packageDirectory = if ($Signing -eq "Local") { $outputRoot } else { Join-Path $signingRoot "unsigned-installer" }
    New-Item -ItemType Directory -Force -Path $packageDirectory | Out-Null
    $package = Join-Path $packageDirectory $installerName
    & $makensis "/DVERSION=$Version" "/DSOURCE_DIR=$releaseDirectory" `
        "/DOUTPUT_FILE=$package" (Join-Path $root "packaging\windows\copypaste.nsi")
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $package -PathType Leaf)) {
        throw "NSIS did not produce the Windows installer"
    }
    if ($Signing -eq "Local") { & $signScript -Operation Sign -File $package }
}

function Complete-Installer {
    if ($Signing -eq "External") {
        New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
        Import-SignedFiles (Join-Path $signingRoot "signed-installer") @{ $installerName = $installer }
    }
    & $signScript -Operation Verify -File $installer

    $digest = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$installer.sha256", "$digest  $installerName`n")
    Write-Host "Built $installerName"
}

Push-Location $root
try {
    if ($Stage -in @("All", "Application")) { Build-Application }
    if ($Stage -in @("All", "Installer")) { Build-Installer }
    if ($Stage -in @("All", "Finalize")) { Complete-Installer }
} finally {
    Pop-Location
}
