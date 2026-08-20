param(
    [switch]$Unsigned,
    [string]$OutputDirectory,
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

if ($env:OS -ne "Windows_NT") { throw "Windows release builds must run on Windows" }
foreach ($command in @("cargo", "npm.cmd", "perl")) { Require-Command $command }
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$uiRoot = Join-Path $repoRoot "crates/copypaste-ui"
$tauriRoot = Join-Path $uiRoot "src-tauri"
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
$config = "src-tauri/tauri.windows.release.conf.json"
$signaturePath = $null
$releaseBaseUrl = $null

try {
    if (-not $Unsigned) {
        $thumbprint = Require-Environment "WINDOWS_CERTIFICATE_THUMBPRINT"
        if ($thumbprint -notmatch '^[0-9A-Fa-f]{40}$') {
            throw "WINDOWS_CERTIFICATE_THUMBPRINT must contain 40 hexadecimal characters"
        }
        $timestampUrl = Require-Environment "WINDOWS_TIMESTAMP_URL"
        $publicKey = Require-Environment "TAURI_UPDATER_PUBLIC_KEY"
        $endpoint = Require-Environment "TAURI_UPDATER_ENDPOINT"
        $releaseBaseUrl = Require-Environment "WINDOWS_RELEASE_BASE_URL"
        Require-Environment "TAURI_SIGNING_PRIVATE_KEY" | Out-Null
        foreach ($url in @($timestampUrl, $endpoint, $releaseBaseUrl)) {
            if (-not $url.StartsWith("https://")) { throw "Release URLs must use HTTPS" }
        }

        $template = Get-Content -Raw -LiteralPath (Join-Path $tauriRoot "tauri.windows.signed.conf.template.json") |
            ConvertFrom-Json
        $template.bundle.windows.certificateThumbprint = $thumbprint
        $template.bundle.windows.timestampUrl = $timestampUrl
        $template.plugins.updater.pubkey = $publicKey
        $template.plugins.updater.endpoints[0] = $endpoint
        $template | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $generatedConfig -Encoding utf8
        $config = "src-tauri/tauri.windows.signed.generated.json"
    }

    Push-Location $repoRoot
    try {
        Invoke-Checked cargo @("build", "--release", "--locked", "-p", "copypaste-cli", "-p", "copypaste-daemon")
    } finally {
        Pop-Location
    }

    $binaryDirectory = Join-Path $tauriRoot "binaries"
    [IO.Directory]::CreateDirectory($binaryDirectory) | Out-Null
    Copy-Item -LiteralPath (Join-Path $repoRoot "target/release/copypaste.exe") -Destination (Join-Path $binaryDirectory "copypaste-$targetTriple.exe") -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot "target/release/copypaste-daemon.exe") -Destination (Join-Path $binaryDirectory "copypaste-daemon-$targetTriple.exe") -Force

    Push-Location $uiRoot
    try {
        Invoke-Checked npm.cmd @("ci")
        Invoke-Checked npm.cmd @("exec", "tauri", "build", "--", "--bundles", "nsis", "--config", $config)
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
}
