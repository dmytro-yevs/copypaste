param(
    [switch]$Unsigned,
    [switch]$SelfTest,
    [string]$OutputDirectory,
    [string]$PrebuiltSidecarsDirectory,
    [ValidateSet("x86_64")]
    [string]$Architecture = "x86_64"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Require-Environment([string]$Name) {
    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value)) { throw "$Name is required for a signed release" }
    return $value
}

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name is required for a Windows release build"
    }
}

function Invoke-Checked([string]$Command, [string[]]$Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command exited $LASTEXITCODE" }
}

function Resolve-PrebuiltSidecars([string]$Directory, [string]$Architecture) {
    $source = [IO.Path]::GetFullPath($Directory)
    $manifestPath = Join-Path $source "sidecars.json"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "Prebuilt sidecar artifact has no architecture manifest"
    }
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    if ($manifest.architecture -ne $Architecture) {
        throw "Prebuilt sidecar architecture does not match $Architecture"
    }
    foreach ($binary in @("copypaste.exe", "copypaste-daemon.exe")) {
        if (-not (Test-Path -LiteralPath (Join-Path $source $binary) -PathType Leaf)) {
            throw "Prebuilt sidecar artifact is incomplete"
        }
    }
    return $source
}

function Write-SignedConfig(
    [string]$TemplatePath,
    [string]$DestinationPath,
    [string]$SignScript,
    [string]$PublicKey,
    [string]$Endpoint
) {
    $config = Get-Content -Raw -LiteralPath $TemplatePath | ConvertFrom-Json
    $config.bundle.windows.signCommand.cmd = "pwsh.exe"
    $config.bundle.windows.signCommand.args = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $SignScript,
        "-Operation",
        "Sign",
        "-File",
        "%1"
    )
    $config.plugins.updater.pubkey = $PublicKey
    $config.plugins.updater.endpoints[0] = $Endpoint
    $config | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $DestinationPath -Encoding utf8
}

function Write-BuildOnlyConfig([string]$SourcePath, [string]$DestinationPath) {
    $config = Get-Content -Raw -LiteralPath $SourcePath | ConvertFrom-Json
    # tauri-build copies these paths while compiling the UI. The release
    # bundle owns that staging after the sidecars exist, so one Cargo graph can
    # compile all three binaries without an ordering race.
    $config.bundle.PSObject.Properties.Remove("externalBin")
    $config.bundle.PSObject.Properties.Remove("resources")
    $config | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $DestinationPath -Encoding utf8
}

function Get-TauriBuildArguments([string]$BuildConfig, [bool]$UsePrebuiltSidecars) {
    $commandArguments = @("exec", "tauri", "build", "--", "--no-bundle", "--config", $BuildConfig, "--", "--locked", "--package", "copypaste-ui")
    if (-not $UsePrebuiltSidecars) {
        $commandArguments += @("--package", "copypaste-cli", "--package", "copypaste-daemon")
    }
    return $commandArguments
}

function Merge-Config([hashtable]$Target, [hashtable]$Patch) {
    foreach ($entry in $Patch.GetEnumerator()) {
        if ($null -eq $entry.Value) {
            $Target.Remove($entry.Key)
        } elseif ($entry.Value -is [System.Collections.IDictionary]) {
            if (-not $Target.ContainsKey($entry.Key) -or $Target[$entry.Key] -isnot [System.Collections.IDictionary]) {
                $Target[$entry.Key] = @{}
            }
            Merge-Config $Target[$entry.Key] $entry.Value
        } else {
            $Target[$entry.Key] = $entry.Value
        }
    }
}

function Invoke-SelfTest {
    $root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-windows-config-self-test-$PID"
    [IO.Directory]::CreateDirectory($root) | Out-Null
    try {
        & (Join-Path $PSScriptRoot "windows-installer-template.ps1") -SelfTest
        & (Join-Path $PSScriptRoot "windows-installer-template.ps1") -Check
        $destination = Join-Path $root "signed.json"
        $signScript = Join-Path $root "windows-sign.ps1"
        Write-SignedConfig `
            (Join-Path $PSScriptRoot "../../crates/copypaste-ui/src-tauri/tauri.windows.signed.conf.template.json") `
            $destination $signScript "self-test-public-key" "https://updates.example.test/latest.json"
        $config = Get-Content -Raw -LiteralPath $destination | ConvertFrom-Json
        $expected = @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $signScript,
            "-Operation", "Sign", "-File", "%1"
        )
        if ($config.bundle.windows.signCommand.cmd -ne "pwsh.exe" -or
            [string]::Join("`n", $config.bundle.windows.signCommand.args) -cne [string]::Join("`n", $expected)) {
            throw "generated signCommand self-test failed"
        }
        if ($config.plugins.updater.pubkey -ne "self-test-public-key" -or
            $config.plugins.updater.endpoints[0] -ne "https://updates.example.test/latest.json") {
            throw "generated updater configuration self-test failed"
        }

        $buildOnly = Join-Path $root "build-only.json"
        Write-BuildOnlyConfig $destination $buildOnly
        $buildConfig = Get-Content -Raw -LiteralPath $buildOnly | ConvertFrom-Json
        if ($null -ne $buildConfig.bundle.PSObject.Properties["externalBin"] -or
            $null -ne $buildConfig.bundle.PSObject.Properties["resources"]) {
            throw "build-only configuration retained bundle staging inputs"
        }
        if ($buildConfig.plugins.updater.pubkey -ne "self-test-public-key" -or
            $buildConfig.plugins.updater.endpoints[0] -ne "https://updates.example.test/latest.json") {
            throw "build-only configuration lost signed updater settings"
        }
        $fullBuild = Get-TauriBuildArguments $buildOnly $false
        $prebuiltBuild = Get-TauriBuildArguments $buildOnly $true
        if ($fullBuild -notcontains "--no-bundle" -or $fullBuild -notcontains "--locked" -or
            [string]::Join("`n", $fullBuild) -notmatch "--package`ncopypaste-ui`n--package`ncopypaste-cli`n--package`ncopypaste-daemon") {
            throw "full Windows build plan does not compile the three release packages once"
        }
        if ($prebuiltBuild -notcontains "--no-bundle" -or
            [string]::Join("`n", $prebuiltBuild) -match "copypaste-cli|copypaste-daemon") {
            throw "prebuilt-sidecar Windows build plan recompiles a sidecar"
        }

        $repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
        $uiRoot = Join-Path $repoRoot "crates/copypaste-ui"
        $unsignedCanonical = Join-Path $uiRoot "src-tauri/tauri.windows.release.conf.json"
        if (-not (Test-Path -LiteralPath $unsignedCanonical -PathType Leaf)) {
            throw "unsigned canonical Windows config is not resolved from the UI root"
        }
        $unsignedBuildOnly = Join-Path $root "unsigned-build-only.json"
        Write-BuildOnlyConfig $unsignedCanonical $unsignedBuildOnly
        $merged = @{}
        foreach ($path in @(
                (Join-Path $uiRoot "src-tauri/tauri.conf.json"),
                (Join-Path $uiRoot "src-tauri/tauri.windows.conf.json"),
                $unsignedBuildOnly
            )) {
            Merge-Config $merged (Get-Content -Raw -LiteralPath $path | ConvertFrom-Json -AsHashtable)
        }
        if ($merged["bundle"].ContainsKey("externalBin") -or $merged["bundle"].ContainsKey("resources")) {
            throw "merged build-only configuration retained bundle staging inputs"
        }
        if ($merged["plugins"]["updater"]["pubkey"] -ne "") {
            throw "merged unsigned build-only configuration changed updater settings"
        }
        $signedMerged = @{}
        foreach ($path in @(
                (Join-Path $uiRoot "src-tauri/tauri.conf.json"),
                (Join-Path $uiRoot "src-tauri/tauri.windows.conf.json"),
                $buildOnly
            )) {
            Merge-Config $signedMerged (Get-Content -Raw -LiteralPath $path | ConvertFrom-Json -AsHashtable)
        }
        if ($signedMerged["bundle"].ContainsKey("externalBin") -or $signedMerged["bundle"].ContainsKey("resources") -or
            $signedMerged["plugins"]["updater"]["pubkey"] -ne "self-test-public-key") {
            throw "merged signed build-only configuration lost its release contract"
        }

        $valid = Join-Path $root "valid"
        [IO.Directory]::CreateDirectory($valid) | Out-Null
        Set-Content -LiteralPath (Join-Path $valid "copypaste.exe") -Value "cli"
        Set-Content -LiteralPath (Join-Path $valid "copypaste-daemon.exe") -Value "daemon"
        @{ architecture = "x86_64" } | ConvertTo-Json |
            Set-Content -LiteralPath (Join-Path $valid "sidecars.json")
        if ((Resolve-PrebuiltSidecars $valid "x86_64") -ne [IO.Path]::GetFullPath($valid)) {
            throw "valid prebuilt sidecar artifact was not resolved"
        }

        $missing = Join-Path $root "missing"
        Copy-Item -LiteralPath $valid -Destination $missing -Recurse
        Remove-Item -LiteralPath (Join-Path $missing "copypaste-daemon.exe")
        try {
            Resolve-PrebuiltSidecars $missing "x86_64" | Out-Null
            throw "unexpected acceptance"
        } catch {
            if ($_.Exception.Message -notlike "*incomplete*") { throw }
        }

        $wrongArchitecture = Join-Path $root "wrong-architecture"
        Copy-Item -LiteralPath $valid -Destination $wrongArchitecture -Recurse
        @{ architecture = "aarch64" } | ConvertTo-Json |
            Set-Content -LiteralPath (Join-Path $wrongArchitecture "sidecars.json")
        try {
            Resolve-PrebuiltSidecars $wrongArchitecture "x86_64" | Out-Null
            throw "unexpected acceptance"
        } catch {
            if ($_.Exception.Message -notlike "*architecture*") { throw }
        }

        Remove-Item -LiteralPath (Join-Path $valid "sidecars.json")
        try {
            Resolve-PrebuiltSidecars $valid "x86_64" | Out-Null
            throw "unexpected acceptance"
        } catch {
            if ($_.Exception.Message -notlike "*manifest*") { throw }
        }
    } finally {
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
    Write-Host "PASS: generated Windows signing configuration contract"
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if ($env:OS -ne "Windows_NT") { throw "Windows release builds must run on Windows" }
foreach ($command in @("cargo", "npm.cmd", "perl")) { Require-Command $command }
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$uiRoot = Join-Path $repoRoot "crates/copypaste-ui"
$tauriRoot = Join-Path $uiRoot "src-tauri"
& (Join-Path $PSScriptRoot "windows-installer-template.ps1") -Check
$package = Get-Content -Raw -LiteralPath (Join-Path $uiRoot "package.json") | ConvertFrom-Json
$metadata = Invoke-Checked cargo @("metadata", "--locked", "--no-deps", "--format-version=1")
$workspaceVersion = ($metadata | ConvertFrom-Json).packages |
    Where-Object { $_.name -eq "copypaste-ui" } |
    Select-Object -ExpandProperty version
if ($package.version -ne $workspaceVersion) {
    throw "package.json version does not match the Rust workspace"
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repoRoot "artifacts/windows-release"
}

$targetTriple = "$Architecture-pc-windows-msvc"
$generatedConfig = Join-Path $tauriRoot "tauri.windows.signed.generated.json"
$buildOnlyDirectory = Join-Path ([IO.Path]::GetTempPath()) "copypaste-windows-build-$PID"
$buildOnlyConfig = Join-Path $buildOnlyDirectory "tauri.windows.build.json"
$config = "src-tauri/tauri.windows.release.conf.json"
$signaturePath = $null
$releaseBaseUrl = $null

try {
    if (-not $Unsigned) {
        $signScript = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "windows-sign.ps1")).Path
        & $signScript -Operation Validate
        $publicKey = Require-Environment "TAURI_UPDATER_PUBLIC_KEY"
        $endpoint = Require-Environment "TAURI_UPDATER_ENDPOINT"
        $releaseBaseUrl = Require-Environment "WINDOWS_RELEASE_BASE_URL"
        Require-Environment "TAURI_SIGNING_PRIVATE_KEY" | Out-Null
        foreach ($url in @($endpoint, $releaseBaseUrl)) {
            if (-not $url.StartsWith("https://")) { throw "Release URLs must use HTTPS" }
        }

        Write-SignedConfig `
            (Join-Path $tauriRoot "tauri.windows.signed.conf.template.json") `
            $generatedConfig $signScript $publicKey $endpoint
        $config = "src-tauri/tauri.windows.signed.generated.json"
    }

    [IO.Directory]::CreateDirectory($buildOnlyDirectory) | Out-Null
    $canonicalConfig = Join-Path $uiRoot $config
    Write-BuildOnlyConfig $canonicalConfig $buildOnlyConfig

    if ($PrebuiltSidecarsDirectory) {
        $sidecarSource = Resolve-PrebuiltSidecars $PrebuiltSidecarsDirectory $Architecture
        $usePrebuiltSidecars = $true
    } else {
        $sidecarSource = Join-Path $repoRoot "target/release"
        $usePrebuiltSidecars = $false
    }

    Push-Location $uiRoot
    try {
        Invoke-Checked npm.cmd @("ci")
        Invoke-Checked npm.cmd (Get-TauriBuildArguments $buildOnlyConfig $usePrebuiltSidecars)
        $binaryDirectory = Join-Path $tauriRoot "binaries"
        [IO.Directory]::CreateDirectory($binaryDirectory) | Out-Null
        Copy-Item -LiteralPath (Join-Path $sidecarSource "copypaste.exe") -Destination (Join-Path $binaryDirectory "copypaste-$targetTriple.exe") -Force
        Copy-Item -LiteralPath (Join-Path $sidecarSource "copypaste-daemon.exe") -Destination (Join-Path $binaryDirectory "copypaste-daemon-$targetTriple.exe") -Force
        Invoke-Checked npm.cmd @("exec", "tauri", "bundle", "--", "--bundles", "nsis", "--config", $config)
    } finally {
        Pop-Location
    }

    $installers = @(Get-ChildItem -LiteralPath (Join-Path $repoRoot "target/release/bundle/nsis") -Filter "*.exe" -File)
    if ($installers.Count -ne 1) { throw "Expected exactly one NSIS installer, found $($installers.Count)" }
    $authenticode = Get-AuthenticodeSignature -LiteralPath $installers[0].FullName
    if ($Unsigned -and $authenticode.Status -ne "NotSigned") {
        throw "Unsigned build unexpectedly has Authenticode status $($authenticode.Status)"
    }
    if (-not $Unsigned -and (
            $null -eq $authenticode.SignerCertificate -or
            $authenticode.Status -notin @("Valid", "UnknownError")
        )) {
        # Self-signed project certs without a trusted root report UnknownError.
        throw "Signed build has Authenticode status $($authenticode.Status)"
    }
    if (-not $Unsigned) {
        $signaturePath = "$($installers[0].FullName).sig"
        if (-not (Test-Path -LiteralPath $signaturePath -PathType Leaf)) {
            throw "Tauri did not create the required updater signature"
        }
        & $signScript -Operation Verify -File (Join-Path $tauriRoot "binaries/copypaste-$targetTriple.exe")
    }

    $packageArguments = @{
        Installer = $installers[0].FullName
        OutputDirectory = $OutputDirectory
        Version = $package.version
        Architecture = $Architecture
    }
    if (-not $Unsigned) {
        $packageArguments.UpdaterSignature = $signaturePath
        $packageArguments.ReleaseBaseUrl = $releaseBaseUrl
    }
    & (Join-Path $PSScriptRoot "package-windows.ps1") @packageArguments
} finally {
    if (Test-Path -LiteralPath $generatedConfig) { [IO.File]::Delete($generatedConfig) }
    if (Test-Path -LiteralPath $buildOnlyDirectory) { [IO.Directory]::Delete($buildOnlyDirectory, $true) }
}
