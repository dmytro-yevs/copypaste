<#
.SYNOPSIS
Run the `Windows workspace + installed product evidence (x64)` CI job here,
before pushing, reusing a cached installer when nothing that feeds it changed.

.DESCRIPTION
Stage names, order and assertions mirror .github/workflows/ci.yml. Unlike CI
every stage runs even after an earlier one fails, except that `installer` and
`evidence` are skipped when the workspace does not compile — there is no point
spending the build on a tree clippy already rejected.

The installer cache is keyed on the content of everything the NSIS bundle is
built from. `-Explain` prints the key and, on a miss, the paths that differ
from the newest cached entry.

.EXAMPLE
scripts/prepush/windows-evidence.ps1

.EXAMPLE
scripts/prepush/windows-evidence.ps1 -Stage installer,evidence -Explain
#>
[CmdletBinding()]
param(
    [string[]]$Stage,
    [string[]]$SkipStage,
    [switch]$ListStages,
    [switch]$ShowCacheKey,
    [switch]$NoCache,
    [switch]$Explain,
    [string]$CacheRoot,
    [int]$KeepCachedBuilds = 5
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Continue"

$RepoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$ToolchainVersion = "1.96"
if (-not $CacheRoot) {
    $CacheRoot = Join-Path $env:LOCALAPPDATA "copypaste-prepush\windows-installer"
}
$PackageDirectory = Join-Path $RepoRoot "artifacts/windows-prepush-package"
$EvidenceDirectory = Join-Path $RepoRoot "artifacts/windows-prepush-native"

# Everything the NSIS bundle is built from. A change to any tracked or
# untracked file under these paths, plus the rustc and node identity below,
# invalidates the cached installer. Nothing else does: docs, .github, e2e,
# tools and the other scripts cannot reach the bundle.
$InstallerInputs = @(
    "Cargo.toml"
    "Cargo.lock"
    "rust-toolchain.toml"
    "crates"
    "design/dist"
    "scripts/release/build-windows.ps1"
    "scripts/release/package-windows.ps1"
)

# ci.yml pins `1.96`, and that is tried first. There is a fallback because a
# rustup toolchain can lose bin/cargo.exe while its component manifest still
# lists cargo, at which point rustup refuses both `component add` (already
# current) and `component remove` (binary missing) and the toolchain cannot be
# repaired in place. The fallback is the exact release `1.96` resolves to, read
# from its own rustc, and it is rejected unless the version string matches: a
# gate that silently runs 1.96.0 while CI runs 1.96.1 is not this gate.
function Resolve-Toolchain {
    $expected = $null
    $release = & rustup run $ToolchainVersion rustc --version 2>&1
    if ($LASTEXITCODE -eq 0 -and (($release -join " ") -match "^rustc (\d+\.\d+\.\d+)")) {
        $expected = ($release -join " ").Trim()
        $candidates = @($ToolchainVersion, $Matches[1])
    } else {
        $candidates = @($ToolchainVersion)
    }

    $failures = @()
    foreach ($candidate in $candidates) {
        $cargo = & rustup which --toolchain $candidate cargo 2>&1
        if ($LASTEXITCODE -ne 0) { $failures += "$candidate : $($cargo | Select-Object -First 1)"; continue }
        # Not piped into `Select-Object -First 1`: early pipeline termination
        # kills the native command and leaves a non-zero $LASTEXITCODE behind.
        $reported = & rustup run $candidate rustc --version 2>&1
        if ($LASTEXITCODE -ne 0) { $failures += "$candidate : $($reported | Select-Object -First 1)"; continue }
        $version = "$(@($reported)[0])"
        if ($expected -and "$version".Trim() -ne $expected) {
            $failures += "$candidate : reports '$version', but $ToolchainVersion is '$expected'"
            continue
        }
        Write-Host "toolchain: $candidate ($version)"
        return $candidate
    }
    throw "no usable rustc $ToolchainVersion toolchain:`n  $($failures -join "`n  ")"
}

function Write-Banner([string]$Text) {
    Write-Host ""
    Write-Host "################ $Text" -ForegroundColor Cyan
}

function Get-GitOutput([string[]]$Arguments) {
    $text = & git -C $RepoRoot @Arguments
    if ($LASTEXITCODE -ne 0) { throw "git $($Arguments -join ' ') exited $LASTEXITCODE" }
    return $text
}

# Content, not mtime: a `git checkout` back and forth rewrites timestamps while
# producing the identical installer, and a cache that misses on that is the
# forty minutes this script exists to remove.
#
# Every hash here is a git blob id, so line endings are normalised the way git
# normalises them. The Tauri build rewrites
# `crates/copypaste-ui/src-tauri/Cargo.toml` from the CRLF git checked out to
# the LF it stores; hashing raw bytes meant every build invalidated its own
# cache entry and the next run rebuilt from scratch. Clean tracked files borrow
# the id already in the index, one process for the whole tree; only what git
# calls dirty or untracked is hashed, by git, so both halves are the same
# function of the same content.
function Get-InstallerInputManifest {
    $dirty = [Collections.Generic.List[string]]::new()
    foreach ($line in @(Get-GitOutput (@("status", "--porcelain", "--untracked-files=all", "--") + $InstallerInputs))) {
        if ($line.Length -lt 4) { continue }
        $path = $line.Substring(3).Trim('"')
        if ($path -match ' -> ') { $path = ($path -split ' -> ')[-1].Trim('"') }
        $dirty.Add($path)
    }

    $records = [Collections.Generic.List[string]]::new()
    foreach ($line in @(Get-GitOutput (@("ls-files", "-s", "--") + $InstallerInputs))) {
        if ($line -notmatch '^\d+ ([0-9a-f]{40}) \d+\t(.+)$') { continue }
        if (-not $dirty.Contains($Matches[2])) { $records.Add("$($Matches[2])`t$($Matches[1])") }
    }
    $live = @($dirty | Where-Object { Test-Path -LiteralPath (Join-Path $RepoRoot $_) -PathType Leaf })
    foreach ($path in $dirty) {
        if ($path -notin $live) { $records.Add("$path`tdeleted") }
    }
    for ($offset = 0; $offset -lt $live.Count; $offset += 100) {
        $batch = @($live[$offset..([Math]::Min($offset + 99, $live.Count - 1))])
        $hashes = @(Get-GitOutput (@("hash-object", "--") + $batch))
        if ($hashes.Count -ne $batch.Count) { throw "git hash-object returned $($hashes.Count) ids for $($batch.Count) paths" }
        for ($i = 0; $i -lt $batch.Count; $i++) { $records.Add("$($batch[$i])`t$($hashes[$i])") }
    }

    $rustc = & rustup run $Toolchain rustc -vV
    if ($LASTEXITCODE -ne 0) { throw "rustup run $Toolchain rustc -vV exited $LASTEXITCODE" }
    $records.Add("!rustc`t$(($rustc -join ' ').Trim())")
    $node = & node --version
    if ($LASTEXITCODE -ne 0) { throw "node --version exited $LASTEXITCODE" }
    $records.Add("!node`t$node")

    $sorted = [string[]]($records | Sort-Object -CaseSensitive)
    return , $sorted
}

function Get-ManifestKey([string[]]$Manifest) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes(($Manifest -join "`n"))
        return [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace("-", "").ToLowerInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Show-CacheMiss([string[]]$Manifest) {
    $previous = @(Get-ChildItem -LiteralPath $CacheRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending)
    if ($previous.Count -eq 0) {
        Write-Output "installer cache: empty, nothing to compare against"
        return
    }
    $manifestPath = Join-Path $previous[0].FullName "inputs.tsv"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { return }
    $old = @{}
    foreach ($line in Get-Content -LiteralPath $manifestPath) {
        $parts = $line -split "`t", 2
        if ($parts.Count -eq 2) { $old[$parts[0]] = $parts[1] }
    }
    $new = @{}
    foreach ($line in $Manifest) {
        $parts = $line -split "`t", 2
        if ($parts.Count -eq 2) { $new[$parts[0]] = $parts[1] }
    }
    $differences = @()
    foreach ($key in $new.Keys) {
        if (-not $old.ContainsKey($key)) { $differences += "added    $key" }
        elseif ($old[$key] -ne $new[$key]) { $differences += "changed  $key" }
    }
    foreach ($key in $old.Keys) {
        if (-not $new.ContainsKey($key)) { $differences += "removed  $key" }
    }
    Write-Output "installer cache: $($differences.Count) input(s) differ from $($previous[0].Name.Substring(0, 12))"
    $differences | Sort-Object | Select-Object -First 20 | ForEach-Object { Write-Output "  $_" }
    if ($differences.Count -gt 20) { Write-Output "  ... and $($differences.Count - 20) more" }
}

function Remove-StaleCacheEntries {
    $entries = @(Get-ChildItem -LiteralPath $CacheRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending)
    if ($entries.Count -le $KeepCachedBuilds) { return }
    foreach ($entry in $entries[$KeepCachedBuilds..($entries.Count - 1)]) {
        Remove-Item -LiteralPath $entry.FullName -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Copy-PackageTo([string]$Source, [string]$Destination) {
    if (Test-Path -LiteralPath $Destination) { Remove-Item -LiteralPath $Destination -Recurse -Force }
    [IO.Directory]::CreateDirectory($Destination) | Out-Null
    foreach ($file in Get-ChildItem -LiteralPath $Source -File) {
        if ($file.Name -in @("inputs.tsv", "key.txt", "built.txt")) { continue }
        Copy-Item -LiteralPath $file.FullName -Destination $Destination -Force
    }
}

function Get-PackagedInstaller {
    $installers = @(Get-ChildItem -LiteralPath $PackageDirectory -Filter "*-setup.exe" -File -ErrorAction SilentlyContinue)
    if ($installers.Count -ne 1) { throw "expected one NSIS installer in $PackageDirectory, found $($installers.Count)" }
    return $installers[0].FullName
}

function Invoke-Installer {
    $manifest = Get-InstallerInputManifest
    $key = Get-ManifestKey $manifest
    if ($Explain) { Write-Host "installer cache key: $key" }
    $entry = Join-Path $CacheRoot $key

    if (-not $NoCache -and (Test-Path -LiteralPath (Join-Path $entry "key.txt"))) {
        Copy-PackageTo $entry $PackageDirectory
        (Get-Item -LiteralPath $entry).LastWriteTime = Get-Date
        Write-Host "installer cache HIT  $key"
        Write-Host (Get-Content -Raw -LiteralPath (Join-Path $entry "built.txt")).Trim()
        Write-Host "reused $(Get-PackagedInstaller)"
        return
    }

    if ($NoCache) { Write-Host "installer cache bypassed (-NoCache)" }
    else {
        Write-Host "installer cache MISS $key"
        Show-CacheMiss $manifest
    }

    Invoke-Script (Join-Path $RepoRoot "scripts/release/build-windows.ps1") @{
        Unsigned        = $true
        OutputDirectory = $PackageDirectory
    }
    Get-PackagedInstaller | Out-Null

    [IO.Directory]::CreateDirectory($entry) | Out-Null
    Copy-PackageTo $PackageDirectory $entry
    Set-Content -LiteralPath (Join-Path $entry "inputs.tsv") -Value $manifest -Encoding utf8
    Set-Content -LiteralPath (Join-Path $entry "key.txt") -Value $key -Encoding utf8
    $head = (Get-GitOutput @("rev-parse", "--short", "HEAD")) -join ""
    Set-Content -LiteralPath (Join-Path $entry "built.txt") -Encoding utf8 -Value @(
        "built $(Get-Date -Format o) on $env:COMPUTERNAME from $head"
    )
    Remove-StaleCacheEntries
    Write-Host "installer cached at $entry"
}

function Invoke-Evidence {
    $installer = Get-PackagedInstaller
    $head = (Get-GitOutput @("rev-parse", "HEAD")) -join ""
    Invoke-Script (Join-Path $RepoRoot "scripts/release/windows-native-evidence.ps1") @{
        Installer         = $installer
        PackageDirectory  = $PackageDirectory
        EvidenceDirectory = $EvidenceDirectory
        ExpectedSignature = "NotSigned"
        Commit            = $head
        RunId             = "prepush-$(Get-Date -Format yyyyMMddHHmmss)"
    }
}

function Invoke-Checked([string]$File, [string[]]$Arguments) {
    $global:LASTEXITCODE = 0
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File $($Arguments -join ' ') exited $LASTEXITCODE" }
}

# Splatting a hashtable, not an array: array splat passes `-SelfTest`
# positionally, so windows-native-evidence.ps1 read the switch name as the
# installer path and the self-test never ran. $LASTEXITCODE is cleared because
# a .ps1 that returns without calling `exit` leaves the previous native
# command's code there, which reads as a failure the callee never had.
function Invoke-Script([string]$Path, [hashtable]$Parameters = @{}) {
    $global:LASTEXITCODE = 0
    & $Path @Parameters
    if ($LASTEXITCODE -ne 0) { throw "$([IO.Path]::GetFileName($Path)) exited $LASTEXITCODE" }
}

# An empty run is a failure, not a pass: a filter that matches nothing exits 0,
# and "we thought Windows was covered" is the shape this stage exists to
# prevent (ci.yml says the same thing at the same place).
function Invoke-Dpapi {
    $log = Join-Path $env:TEMP "copypaste-prepush-$PID-dpapi.log"
    & cargo "+$Toolchain" test -p copypaste-core --locked crypto::keystore:: -- --nocapture 2>&1 |
        Tee-Object -FilePath $log
    $status = $LASTEXITCODE
    if ($status -ne 0) { throw "the device-secret tests exited $status" }
    if (-not (Select-String -LiteralPath $log -Pattern 'test result: ok\. [1-9][0-9]* passed' -Quiet)) {
        throw "no device-secret test ran"
    }
}

# The backend is DPAPI precisely because a credential in this store is
# enumerable by anything in the session.
function Invoke-CredentialManager {
    $found = cmdkey /list | Select-String -Pattern 'copypaste' -SimpleMatch
    if ($found) {
        $found | ForEach-Object { Write-Host $_ }
        throw "a CopyPaste credential is in the Credential Manager"
    }
    Write-Host "no CopyPaste entry in the Credential Manager, as designed"
}

$Stages = [ordered]@{
    "perl" = @{
        Needs = @()
        Run   = { Invoke-Checked $env:OPENSSL_SRC_PERL @("-MLocale::Maketext::Simple", "-e", "exit 0") }
    }
    "clippy" = @{
        Needs = @()
        Run   = { Invoke-Checked "cargo" @("+$Toolchain", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings") }
    }
    "test" = @{
        Needs = @()
        Run   = { Invoke-Checked "cargo" @("+$Toolchain", "test", "--workspace", "--locked") }
    }
    "pydeps" = @{
        Needs = @()
        Run   = { Invoke-Checked "python" @("-m", "pip", "install", "--requirement", "requirements-ci.txt") }
    }
    "frontend" = @{
        Needs = @()
        Run   = {
            Push-Location (Join-Path $RepoRoot "crates/copypaste-ui")
            try {
                Invoke-Checked "npm.cmd" @("ci")
                Invoke-Checked "npm.cmd" @("test")
                Invoke-Checked "npm.cmd" @("run", "build")
            } finally { Pop-Location }
        }
    }
    "dpapi"    = @{ Needs = @();                    Run = { Invoke-Dpapi } }
    "credmgr"  = @{ Needs = @();                    Run = { Invoke-CredentialManager } }
    "fixtures" = @{
        Needs = @()
        Run   = {
            Invoke-Script (Join-Path $RepoRoot "scripts/release/test-package-windows.ps1")
            Invoke-Script (Join-Path $RepoRoot "scripts/release/windows-native-evidence.ps1") @{ SelfTest = $true }
        }
    }
    "installer" = @{ Needs = @("clippy", "test", "frontend"); Run = { Invoke-Installer } }
    "evidence"  = @{ Needs = @("installer", "pydeps");        Run = { Invoke-Evidence } }
}

if ($ListStages) {
    foreach ($name in $Stages.Keys) { Write-Output $name }
    exit 0
}
if ($env:OS -ne "Windows_NT") { throw "this gate is the Windows CI job; run it on Windows" }
$Toolchain = Resolve-Toolchain
# ci.yml sets both at job level. RUSTUP_TOOLCHAIN is what makes the bare
# `cargo` inside build-windows.ps1 and the Tauri build use the pinned compiler
# instead of rust-toolchain.toml's `stable`.
$env:RUSTUP_TOOLCHAIN = $Toolchain
if (-not $env:OPENSSL_SRC_PERL) { $env:OPENSSL_SRC_PERL = 'C:\Strawberry\perl\bin\perl.exe' }
if ($ShowCacheKey) {
    $manifest = Get-InstallerInputManifest
    $key = Get-ManifestKey $manifest
    Write-Output "installer cache key: $key"
    if (Test-Path -LiteralPath (Join-Path (Join-Path $CacheRoot $key) "key.txt")) {
        Write-Output "installer cache HIT  $key"
    } else {
        Write-Output "installer cache MISS $key"
        Show-CacheMiss $manifest
    }
    exit 0
}
foreach ($name in @($Stage) + @($SkipStage)) {
    if ($name -and -not $Stages.Contains($name)) { throw "unknown stage '$name'; see -ListStages" }
}

$selected = if ($Stage) { @($Stages.Keys | Where-Object { $_ -in $Stage }) } else { @($Stages.Keys) }
if ($SkipStage) { $selected = @($selected | Where-Object { $_ -notin $SkipStage }) }

$results = [ordered]@{}
$overall = 0
$total = [Diagnostics.Stopwatch]::StartNew()
Push-Location $RepoRoot
try {
    foreach ($name in $selected) {
        $blocked = @($Stages[$name].Needs | Where-Object { $results[$_] -eq "FAIL" -or $results[$_] -eq "SKIP" })
        if ($blocked.Count -gt 0) {
            Write-Banner "$name (skipped)"
            Write-Host "skipped: $($blocked -join ', ') did not pass"
            $results[$name] = "SKIP"
            $overall = 1
            continue
        }
        Write-Banner $name
        $timer = [Diagnostics.Stopwatch]::StartNew()
        try {
            & $Stages[$name].Run
            $results[$name] = "PASS"
        } catch {
            # With the position: a strict-mode property error names neither the
            # script nor the line in its message, and the failing stage is often
            # inside scripts/release.
            Write-Host "FAILED: $($_.Exception.Message)" -ForegroundColor Red
            if ($_.InvocationInfo) { Write-Host $_.InvocationInfo.PositionMessage -ForegroundColor Red }
            $results[$name] = "FAIL"
            $overall = 1
        }
        $timer.Stop()
        Write-Host ("{0} {1} in {2:n0}s" -f $results[$name], $name, $timer.Elapsed.TotalSeconds)
    }
} finally {
    Pop-Location
}
$total.Stop()

Write-Host ""
Write-Host "================ SUMMARY  windows-evidence  $(Get-GitOutput @('rev-parse', '--abbrev-ref', 'HEAD'))"
foreach ($name in $results.Keys) { Write-Host ("{0}  {1}" -f $results[$name], $name) }
Write-Host ("total {0:n0}s" -f $total.Elapsed.TotalSeconds)
exit $overall
