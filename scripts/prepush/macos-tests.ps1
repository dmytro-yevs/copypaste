<#
.SYNOPSIS
Run the macOS half of `macOS check + platform (arm64)` on the paired Apple
Silicon host before pushing, against the commit you are about to push.

.DESCRIPTION
Nothing is pushed anywhere. The commit is served straight off this machine by a
read-only `git daemon` bound to the LAN interface the Mac reaches, fetched into
a scratch checkout there, and tested. The scratch checkout keeps its own
`target/`, so the second run on the same host is incremental.

Requires the Orca CLI and a paired environment (`orca environment list`).

.EXAMPLE
scripts/prepush/macos-tests.ps1

.EXAMPLE
scripts/prepush/macos-tests.ps1 -Ref origin/main -CargoArguments 'clippy --workspace --all-targets --all-features --locked -- -D warnings'
#>
[CmdletBinding()]
param(
    [string]$Environment = "MAC",
    [string]$Ref = "HEAD",
    [string]$RemoteRoot = "/Users/dmytro/orca/prepush/copypaste",
    [string]$AnchorWorktree = "path:/Users/dmytro/Documents/CopyPaste",
    [string]$RemoteToolchain = "1.96",
    [string]$CargoArguments = "test --workspace --locked",
    [int]$Port = 9419,
    [string]$ListenAddress,
    [int]$TimeoutMinutes = 120,
    [int]$PollSeconds = 20,
    [switch]$KeepTerminal
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$Done = "__PREPUSH_DONE__"

function Invoke-Git([string[]]$Arguments) {
    $text = & git -C $RepoRoot @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw "git $($Arguments -join ' ') exited $LASTEXITCODE`n$($text -join "`n")" }
    return $text
}

function Send-Line([string]$Handle, [string]$Text) {
    $reply = & orca --environment $Environment terminal send --terminal $Handle --text $Text --enter --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "terminal send failed:`n$($reply -join "`n")" }
    Start-Sleep -Milliseconds 400
}

# Two constraints, both learned the hard way:
#
#   * The Orca CLI parses `--`-prefixed tokens out of the middle of `--text`,
#     so `cargo test --workspace` sent literally arrives as a broken command
#     line. Base64's alphabet has no hyphen, so an encoded payload survives.
#   * A tty in canonical mode discards input past its line limit. The whole
#     encoded script on one line came to ~960 bytes and arrived clipped to
#     `... | base6`, which zsh then reported as a missing command. Chunked
#     appends keep every line far below that.
function Invoke-Remote([string]$Handle, [string]$Script, [string]$Staging) {
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($Script))
    Send-Line $Handle ": > $Staging.b64"
    for ($offset = 0; $offset -lt $encoded.Length; $offset += 256) {
        $chunk = $encoded.Substring($offset, [Math]::Min(256, $encoded.Length - $offset))
        Send-Line $Handle "printf %s '$chunk' >> $Staging.b64"
    }
    Send-Line $Handle "base64 -d < $Staging.b64 > $Staging.sh && bash $Staging.sh; rm -f $Staging.b64 $Staging.sh"
}

# `terminal create` answers before the shell can run anything, and input sent
# into that window sits at the prompt unexecuted. Readiness is proved by a
# marker that came back, not by a prompt that was drawn; a stalled first line
# is freed with one Enter and never by resending it.
function Wait-Ready([string]$Handle, [int]$Attempts = 4) {
    $marker = "RDY$([guid]::NewGuid().ToString('N').Substring(0, 8))"
    Send-Line $Handle "echo $marker"
    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        for ($tick = 0; $tick -lt 6; $tick++) {
            Start-Sleep -Seconds 1
            $terminal = Read-Remote $Handle "0"
            if (@($terminal.tail) | Where-Object { $_.Trim() -eq $marker }) { return }
        }
        Send-Line $Handle ""
    }
    throw "the remote shell never echoed $marker"
}

function Read-Remote([string]$Handle, [string]$Cursor) {
    $reply = & orca --environment $Environment terminal read --terminal $Handle `
        --cursor $Cursor --limit 500 --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "terminal read failed:`n$($reply -join "`n")" }
    return ($reply -join "`n" | ConvertFrom-Json).result.terminal
}

function Get-RemoteAddress {
    $reply = & orca environment list --json 2>&1
    $environments = ($reply -join "`n" | ConvertFrom-Json).result.environments
    $match = @($environments | Where-Object { $_.name -eq $Environment -or $_.id -eq $Environment })
    if ($match.Count -eq 0) { throw "no saved Orca environment named '$Environment'" }
    foreach ($endpoint in @($match[0].endpoints)) {
        if ("$($endpoint.endpoint)" -match '://([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)') { return $Matches[1] }
    }
    throw "environment '$Environment' publishes no IPv4 endpoint"
}

# Longest matching prefix, not `Find-NetRoute`: this host also carries Tailscale
# and ZeroTier addresses, and Find-NetRoute answers a LAN address with the
# Tailscale interface, which the Mac's route to us does not use.
function Get-ListenAddress([string]$Remote) {
    if ($ListenAddress) { return $ListenAddress }
    $remoteBytes = ([Net.IPAddress]::Parse($Remote)).GetAddressBytes()
    $best = $null
    $bestScore = -1
    foreach ($candidate in Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -ne "127.0.0.1" }) {
        $bytes = ([Net.IPAddress]::Parse($candidate.IPAddress)).GetAddressBytes()
        $score = 0
        while ($score -lt 4 -and $bytes[$score] -eq $remoteBytes[$score]) { $score++ }
        if ($score -gt $bestScore) {
            $bestScore = $score
            $best = $candidate.IPAddress
        }
    }
    if (-not $best) { throw "no local IPv4 interface to serve from" }
    return $best
}

$remoteAddress = Get-RemoteAddress
$listen = Get-ListenAddress $remoteAddress
Write-Host "mac:     $remoteAddress"
Write-Host "serving: git://${listen}:$Port"

$commit = (Invoke-Git @("rev-parse", "--verify", "$Ref^{commit}")) -join ""
$dirty = @(Invoke-Git @("status", "--porcelain")).Where({ $_ }).Count
Write-Host "commit:  $commit"
if ($dirty -gt 0) {
    Write-Host "WARNING: $dirty uncommitted path(s) are NOT part of this run; it tests $Ref as committed" -ForegroundColor Yellow
}

$commonDir = ((Invoke-Git @("rev-parse", "--path-format=absolute", "--git-common-dir")) -join "").Trim()
$servedName = [IO.Path]::GetFileName([IO.Path]::GetDirectoryName($commonDir.TrimEnd('/', '\')))
$baseParent = [IO.Path]::GetDirectoryName([IO.Path]::GetDirectoryName($commonDir.TrimEnd('/', '\'))) -replace '\\', '/'
$servedRef = "refs/prepush/$([guid]::NewGuid().ToString('N').Substring(0, 12))"
$url = "git://${listen}:$Port/$servedName/.git"

$daemon = $null
$handle = $null
$exitCode = 1
try {
    Invoke-Git @("update-ref", $servedRef, $commit) | Out-Null
    $daemon = Start-Process -FilePath (Get-Command git).Source -WindowStyle Hidden -PassThru -ArgumentList @(
        "daemon"
        "--reuseaddr"
        "--listen=$listen"
        "--port=$Port"
        "--base-path=$baseParent"
        "--export-all"
        "--informative-errors"
        $commonDir
    )
    Start-Sleep -Seconds 2
    if ($daemon.HasExited) { throw "git daemon exited with code $($daemon.ExitCode)" }

    $create = & orca --environment $Environment terminal create --worktree $AnchorWorktree `
        --title "prepush-macos" --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "terminal create failed:`n$($create -join "`n")" }
    $handle = ($create -join "`n" | ConvertFrom-Json).result.terminal.handle
    Write-Host "terminal: $handle"
    Wait-Ready $handle

    $script = @"
set -u
. "`$HOME/.cargo/env" 2>/dev/null || true
mkdir -p '$RemoteRoot' || exit 90
cd '$RemoteRoot' || exit 90
[ -d .git ] || git init -q || exit 91
git fetch -q --no-tags '$url' "+${servedRef}:${servedRef}" || exit 92
git checkout -q --detach '$servedRef' || exit 93
git reset -q --hard '$servedRef' || exit 93
head="`$(git rev-parse HEAD)"
if [ "`$head" != '$commit' ]; then echo "fetched `$head, expected $commit"; exit 94; fi
echo "==== $RemoteRoot at `$head"
sw_vers
cargo '+$RemoteToolchain' $CargoArguments
echo "$Done rc=`$?"
"@

    $cursor = (Read-Remote $handle "0").nextCursor
    Invoke-Remote $handle $script "/tmp/copypaste-prepush-$([guid]::NewGuid().ToString('N').Substring(0, 8))"
    $deadline = (Get-Date).AddMinutes($TimeoutMinutes)
    $finished = $false
    while (-not $finished) {
        Start-Sleep -Seconds $PollSeconds
        $terminal = Read-Remote $handle $cursor
        $cursor = $terminal.nextCursor
        foreach ($line in @($terminal.tail)) {
            Write-Host $line
            # The daemon exposes every ref in the shared repository to the LAN
            # while it is up, so it comes down the moment the checkout line
            # proves the Mac no longer needs it.
            if ($line -match "^==== " -and $daemon -and -not $daemon.HasExited) {
                Stop-Process -Id $daemon.Id -Force
                Write-Host "git daemon stopped; the Mac has the commit"
            }
            if ($line -match "$Done rc=(\d+)") {
                $exitCode = [int]$Matches[1]
                $finished = $true
            }
        }
        if ($finished) { break }
        if ($terminal.status -ne "running") { throw "the remote terminal reported status '$($terminal.status)' before finishing" }
        if ((Get-Date) -gt $deadline) { throw "the remote run did not finish within $TimeoutMinutes minutes" }
    }
} finally {
    if ($handle -and -not $KeepTerminal) {
        & orca --environment $Environment terminal close --terminal $handle --tab --json 2>&1 | Out-Null
    }
    if ($daemon -and -not $daemon.HasExited) { Stop-Process -Id $daemon.Id -Force }
    & git -C $RepoRoot update-ref -d $servedRef 2>&1 | Out-Null
}

$setupFailures = @{
    90 = "could not create $RemoteRoot"
    91 = "git init failed in $RemoteRoot"
    92 = "the Mac could not fetch $url; pass -ListenAddress with an interface it can reach"
    93 = "checkout of $servedRef failed"
    94 = "the Mac checked out a different commit"
}

Write-Host ""
if ($exitCode -eq 0) {
    Write-Host "PASS: cargo $CargoArguments on $Environment at $commit"
} elseif ($setupFailures.ContainsKey($exitCode)) {
    Write-Host "FAIL: $($setupFailures[$exitCode]) (exit $exitCode)" -ForegroundColor Red
} else {
    Write-Host "FAIL: cargo $CargoArguments on $Environment at $commit exited $exitCode" -ForegroundColor Red
}
exit $exitCode
