param(
    [string]$Installer,
    [string]$PackageDirectory,
    [string]$EvidenceDirectory,
    [ValidateSet("NotSigned", "Valid")]
    [string]$ExpectedSignature = "NotSigned",
    [string]$Commit = $env:GITHUB_SHA,
    [string]$RunId = $env:GITHUB_RUN_ID,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "windows-native-ui-evidence.ps1")
. (Join-Path $PSScriptRoot "windows-process-trace.ps1")

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Assert-InstalledLayout([string]$Directory) {
    foreach ($name in @("copypaste-ui.exe", "copypaste.exe", "copypaste-daemon.exe", "uninstall.exe")) {
        Assert-True (Test-Path -LiteralPath (Join-Path $Directory $name) -PathType Leaf) "installed package is missing $name"
    }
}

function Invoke-Json([string]$Cli, [string[]]$Arguments) {
    $text = & $Cli --json @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw ($text -join "`n") }
    $reply = ($text -join "`n") | ConvertFrom-Json
    Assert-True ($reply.ok -eq $true) "CLI returned a refusal"
    return $reply
}

# `CliError::DaemonUnreachable` is exit 1; a daemon still holding the pipe
# answers 0. Nothing else here can tell an orphaned sidecar from a stopped one.
function Assert-Unreachable([string]$Cli) {
    & $Cli --json status | Out-Null
    Assert-True ($LASTEXITCODE -eq 1) "the CLI answered exit $LASTEXITCODE with no sidecar running"
}

function Assert-PackageIntegrity([string]$Directory, [string]$InstallerPath, [string]$SigningStatus) {
    $checksumsPath = Join-Path $Directory "SHA256SUMS"
    Assert-True (Test-Path -LiteralPath $checksumsPath -PathType Leaf) "Windows package is missing SHA256SUMS"
    $name = [IO.Path]::GetFileName($InstallerPath)
    $signaturePath = "$InstallerPath.sig"
    $latestPath = Join-Path $Directory "latest.json"
    $expectedAssets = @(if ($SigningStatus -eq "Valid") { $name; "$name.sig"; "latest.json" } else { $name })
    $observedAssets = foreach ($line in @(Get-Content -LiteralPath $checksumsPath)) {
        Assert-True ($line -match '^([0-9A-Fa-f]{64})  ([^\\/]+)$') "SHA256SUMS contains an invalid entry"
        $assetName = $Matches[2]
        Assert-True ($assetName -in $expectedAssets) "SHA256SUMS contains unexpected asset $assetName"
        $assetPath = Join-Path $Directory $assetName
        Assert-True (Test-Path -LiteralPath $assetPath -PathType Leaf) "checksummed asset is missing: $assetName"
        $actual = (Get-FileHash -LiteralPath $assetPath -Algorithm SHA256).Hash.ToLowerInvariant()
        Assert-True ($actual -eq $Matches[1].ToLowerInvariant()) "SHA256SUMS does not authenticate $assetName"
        $assetName
    }
    Assert-True (@($observedAssets).Count -eq $expectedAssets.Count) "SHA256SUMS has the wrong number of Windows assets"
    Assert-True (@($expectedAssets | Where-Object { $_ -notin $observedAssets }).Count -eq 0) "SHA256SUMS omits a Windows asset"

    if ($SigningStatus -eq "Valid") {
        Assert-True (Test-Path -LiteralPath $signaturePath -PathType Leaf) "signed package is missing its updater signature"
        Assert-True (Test-Path -LiteralPath $latestPath -PathType Leaf) "signed package is missing latest.json"
        $signature = (Get-Content -Raw -LiteralPath $signaturePath).Trim()
        $latest = Get-Content -Raw -LiteralPath $latestPath | ConvertFrom-Json
        Assert-True ($name -match '^CopyPaste-v(.+)-windows-x86_64-setup\.exe$') "installer filename is not canonical"
        Assert-True ($latest.version -eq $Matches[1]) "latest.json names a different version"
        Assert-True (@($latest.platforms.PSObject.Properties).Count -eq 1) "latest.json must contain one Windows platform"
        $platform = $latest.platforms.'windows-x86_64'
        Assert-True ($platform.signature -eq $signature) "latest.json has a different updater signature"
        $url = [uri]$platform.url
        Assert-True ($url.Scheme -eq "https") "latest.json updater URL must use HTTPS"
        Assert-True ([IO.Path]::GetFileName($url.AbsolutePath) -eq $name) "latest.json names a different installer"
    } else {
        Assert-True (-not (Test-Path -LiteralPath $signaturePath)) "unsigned package carries an updater signature"
        Assert-True (-not (Test-Path -LiteralPath $latestPath)) "unsigned package carries releasable update metadata"
    }
}

# A CLI that cannot reach a daemon still binding its endpoint is the expected
# state on the way to ready. Anything else is reported on the probe that saw it
# rather than only in the final timeout line.
function Get-CliProbeOutcome([string]$Failure) {
    # `cannot reach the CopyPaste daemon` is CliError::DaemonUnreachable's exact
    # user_message; `not_ready` is ErrorCode::NotReady on the --json envelope.
    if ($Failure -match "cannot reach the CopyPaste daemon|not_ready|offline|refused|No connection could be made|cannot find the file") {
        return New-ProbeNotReady "the CLI has not reached the daemon yet: $Failure"
    }
    return New-ProbeTransient "the CLI failed: $Failure"
}

function Get-InstalledSidecarOutcome([string]$DaemonExe, $Processes) {
    $candidates = if ($null -eq $Processes) {
        @(Get-Process -Name "copypaste-daemon" -ErrorAction SilentlyContinue)
    } else {
        @($Processes)
    }
    $matching = @()
    $unobservable = 0
    $wrongPath = 0
    foreach ($candidate in $candidates) {
        try { $path = $candidate.Path } catch { $path = $null }
        if ([string]::IsNullOrEmpty($path)) {
            $unobservable++
        } elseif ($path -eq $DaemonExe) {
            $matching += $candidate
        } else {
            $wrongPath++
        }
    }
    if ($wrongPath -gt 0) {
        return New-ProbeInvariant "$wrongPath daemon process(es) are running outside the installed sidecar path"
    }
    if ($matching.Count -gt 1) { return New-ProbeInvariant "the installed sidecar count is $($matching.Count), not 1" }
    if ($unobservable -gt 0) {
        return New-ProbeTransient "$unobservable daemon process path(s) are not observable yet"
    }
    if ($matching.Count -eq 1) { return New-ProbeReady $matching[0] }
    return New-ProbeNotReady "the installed sidecar count is 0, not 1"
}

function Get-BoundedRuntimeLog([string]$DataRoot, [string]$Process, [int]$MaxChars = 65536) {
    $logDirectory = Join-Path $DataRoot "logs"
    $lines = @(
        Get-ChildItem -LiteralPath $logDirectory -Filter "$Process.*.log" -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTimeUtc |
            ForEach-Object { Get-Content -LiteralPath $_.FullName -Tail 2000 -ErrorAction SilentlyContinue }
    )
    if ($lines.Count -eq 0) { return "<no $Process runtime events>" }
    $text = $lines -join "`n"
    if ($text.Length -gt $MaxChars) {
        $prefix = "<truncated>`n"
        $bodyChars = [Math]::Max(0, $MaxChars - $prefix.Length)
        $text = $prefix + $text.Substring($text.Length - $bodyChars)
    }
    return $text
}

function Save-WindowsFailureDiagnostics([string]$DataRoot, [string]$TracePath, [string]$EvidencePath) {
    $failureDirectory = Join-Path $EvidencePath "failure-diagnostics"
    [IO.Directory]::CreateDirectory($failureDirectory) | Out-Null
    Get-BoundedRuntimeLog $DataRoot "app" 262144 |
        Set-Content -LiteralPath (Join-Path $failureDirectory "app.log") -Encoding utf8
    Get-BoundedRuntimeLog $DataRoot "daemon" 262144 |
        Set-Content -LiteralPath (Join-Path $failureDirectory "daemon.log") -Encoding utf8
    if (Test-Path -LiteralPath $TracePath -PathType Leaf) {
        Copy-Item -LiteralPath $TracePath -Destination (Join-Path $failureDirectory "process-trace.jsonl") -Force
    } else {
        '{"kind":"trace-unavailable","reason":"file-missing"}' |
            Set-Content -LiteralPath (Join-Path $failureDirectory "process-trace.jsonl") -Encoding utf8
    }
}

function Invoke-FailureDiagnosticPersistence {
    [CmdletBinding()]
    param(
        [AllowNull()]
        [System.Management.Automation.ErrorRecord]$PendingFailure,
        [ValidateSet("process-trace", "artifact-save")]
        [string]$Operation,
        [scriptblock]$Action
    )
    try {
        & $Action
    } catch {
        if ($null -ne $PendingFailure) {
            $key = "CopyPaste.DiagnosticsPersistence"
            $previous = [string]$PendingFailure.Exception.Data[$key]
            $annotations = @($previous, $Operation) |
                Where-Object { $_ } |
                Select-Object -Unique
            $PendingFailure.Exception.Data[$key] = $annotations -join ","
            Write-Warning "Failure diagnostics $Operation failed; preserving the original failure."
        } else {
            Write-Warning "Failure diagnostics $Operation failed."
        }
    }
}

function Get-InstalledDiagnostics([Diagnostics.Process]$App, [string]$DataRoot, [string]$DaemonError) {
    $parts = @()
    if ($App) {
        $App.Refresh()
        $parts += if ($App.HasExited) { "app exited with code $($App.ExitCode)" } else { "app is running" }
        try { $parts += Get-UiaSummary $App } catch { $parts += "UIA: $($_.Exception.Message)" }
    }
    if (Test-Path -LiteralPath $DaemonError -PathType Leaf) {
        $stderr = [IO.File]::ReadAllText($DaemonError).Trim()
        $parts += if ($stderr) { "daemon stderr: <present; content omitted>" } else { "daemon stderr: <empty>" }
    }
    $parts += "app runtime log: $(Get-BoundedRuntimeLog $DataRoot "app")"
    $parts += "daemon runtime log: $(Get-BoundedRuntimeLog $DataRoot "daemon")"
    return $parts
}

function Invoke-SelfTest {
    $root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-windows-evidence-self-test-$([guid]::NewGuid())"
    [IO.Directory]::CreateDirectory($root) | Out-Null
    try {
        Test-WindowsReadinessHelpers
        Test-WindowsProcessTraceHelpers
        Test-WindowsProcessTraceCollector

        $logs = Join-Path $root "logs"
        [IO.Directory]::CreateDirectory($logs) | Out-Null
        [IO.File]::WriteAllText((Join-Path $logs "app.fixture.log"), ("a" * 300000))
        [IO.File]::WriteAllText((Join-Path $logs "daemon.fixture.log"), "daemon lifecycle")
        $traceFixture = Join-Path $root "process-trace.jsonl"
        [IO.File]::WriteAllText($traceFixture, '{"kind":"start","pid":41}')
        Save-WindowsFailureDiagnostics $root $traceFixture $root
        $failureFixture = Join-Path $root "failure-diagnostics"
        $boundedApp = [IO.File]::ReadAllText((Join-Path $failureFixture "app.log"))
        Assert-True ($boundedApp.Length -le 262150) "failure app log was not bounded"
        Assert-True (Test-Path -LiteralPath (Join-Path $failureFixture "daemon.log")) "failure daemon log was not separate"
        Assert-True (Test-Path -LiteralPath (Join-Path $failureFixture "process-trace.jsonl")) "failure process trace was not preserved"

        $pending = try { throw "original pending failure" } catch { $_ }
        $warnings = @()
        Invoke-FailureDiagnosticPersistence $pending "artifact-save" {
            throw "C:\Users\private\diagnostic-copy-failed"
        } -WarningVariable +warnings
        $rethrown = try { throw $pending } catch { $_ }
        Assert-True ($rethrown.Exception.Message -eq "original pending failure") "diagnostic save replaced the pending failure"
        Assert-True ($pending.Exception.Data["CopyPaste.DiagnosticsPersistence"] -eq "artifact-save") "diagnostic save failure was not annotated"
        Assert-True (-not (($warnings -join "`n") -match "private|diagnostic-copy-failed")) "diagnostic save warning exposed its error"

        foreach ($name in @("copypaste-ui.exe", "copypaste.exe", "uninstall.exe")) {
            [IO.File]::WriteAllText((Join-Path $root $name), "fixture")
        }
        $rejected = $false
        try { Assert-InstalledLayout $root } catch { $rejected = $_.Exception.Message -match "copypaste-daemon.exe" }
        Assert-True $rejected "a package without the installed sidecar did not fail"

        # A daemon that survived the app answers 0, which is the orphan the
        # assertion exists to catch; a CLI that cannot be run at all is not.
        $answering = Join-Path $root "still-answering.cmd"
        [IO.File]::WriteAllText($answering, "@exit /b 0`r`n")
        $rejected = $false
        try { Assert-Unreachable $answering } catch { $rejected = $_.Exception.Message -match "exit 0" }
        Assert-True $rejected "a sidecar still answering was accepted as a stopped one"
        $refusing = Join-Path $root "refusing.cmd"
        [IO.File]::WriteAllText($refusing, "@exit /b 1`r`n")
        Assert-Unreachable $refusing

        $emptyError = Join-Path $root "empty-daemon.stderr.log"
        [IO.File]::WriteAllText($emptyError, "")
        $diagnostics = @(Get-InstalledDiagnostics $null $root $emptyError)
        Assert-True ($diagnostics -contains "daemon stderr: <empty>") "empty daemon stderr broke diagnostics"
        [IO.File]::WriteAllText($emptyError, "C:\Users\private\secret.db")
        $diagnostics = @(Get-InstalledDiagnostics $null $root $emptyError)
        Assert-True (-not (($diagnostics -join "`n") -match "private|secret.db")) "daemon stderr exposed a path"

        $seen = @{ probes = 0 }
        $sidecar = Wait-Readiness "a transiently unobservable sidecar" {
            $seen.probes++
            $processes = @([pscustomobject]@{ Path = "C:\Program Files\CopyPaste\copypaste-daemon.exe" })
            if ($seen.probes -eq 1) { $processes += [pscustomobject]@{ Path = $null } }
            Get-InstalledSidecarOutcome "C:\Program Files\CopyPaste\copypaste-daemon.exe" $processes
        } { "fixture diagnostics" } 1000 2 1 250 250 { param($ms) }
        Assert-True ($sidecar.Path -like "*copypaste-daemon.exe") "a transient process-path race did not recover"
        Assert-True ($seen.probes -eq 2) "a correct sidecar hid an unobservable duplicate"

        $seen.probes = 0
        $rejected = $false
        try {
            Wait-Readiness "a wrong-path sidecar" {
                $seen.probes++
                Get-InstalledSidecarOutcome "C:\Program Files\CopyPaste\copypaste-daemon.exe" @(
                    [pscustomobject]@{ Path = "C:\Program Files\CopyPaste\copypaste-daemon.exe" },
                    [pscustomobject]@{ Path = "C:\Other\copypaste-daemon.exe" }
                )
            } { "fixture diagnostics" } 1000 2 1 250 250 { param($ms) } | Out-Null
        } catch {
            $rejected = $_.Exception.Message -match "outside the installed sidecar path"
        }
        Assert-True ($rejected -and $seen.probes -eq 1) "a wrong-path duplicate did not fail closed immediately"

        $duplicate = Get-InstalledSidecarOutcome "C:\Program Files\CopyPaste\copypaste-daemon.exe" @(
            [pscustomobject]@{ Path = "C:\Program Files\CopyPaste\copypaste-daemon.exe" },
            [pscustomobject]@{ Path = "c:\program files\copypaste\copypaste-daemon.exe" }
        )
        Assert-True ($duplicate.kind -eq "invariant") "duplicate correct sidecars were accepted"

        $rejected = $false
        try {
            Wait-Readiness "a genuinely missing sidecar" {
                Get-InstalledSidecarOutcome "C:\Program Files\CopyPaste\copypaste-daemon.exe" @()
            } { "fixture diagnostics" } 1000 2 1 250 250 { param($ms) } | Out-Null
        } catch {
            $rejected = $_.Exception.Message -match "sidecar count is 0" -and
                        $_.Exception.Message -match "fixture diagnostics"
        }
        Assert-True $rejected "a zero-sidecar candidate set did not remain a bounded failure"

        Test-WindowsUiEvidenceHelpers
        Write-Output "PASS: a broken installed sidecar package fails closed"
        Write-Output "PASS: an orphaned sidecar fails the shutdown assertion"
    } finally {
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}
if (-not $Installer) { throw "Installer is required" }

$evidencePath = $null
if ($EvidenceDirectory) {
    $evidencePath = [IO.Path]::GetFullPath($EvidenceDirectory)
    $failureDirectory = Join-Path $evidencePath "failure-diagnostics"
    [IO.Directory]::CreateDirectory($failureDirectory) | Out-Null
    "<no app runtime events>" | Set-Content -LiteralPath (Join-Path $failureDirectory "app.log") -Encoding utf8
    "<no daemon runtime events>" | Set-Content -LiteralPath (Join-Path $failureDirectory "daemon.log") -Encoding utf8
    '{"kind":"trace-unavailable","reason":"not-started"}' |
        Set-Content -LiteralPath (Join-Path $failureDirectory "process-trace.jsonl") -Encoding utf8
}

$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$packagePath = if ($PackageDirectory) {
    (Resolve-Path -LiteralPath $PackageDirectory).Path
} else {
    Split-Path -Parent $installerPath
}
$signature = Get-AuthenticodeSignature -FilePath $installerPath
if ($ExpectedSignature -eq "NotSigned") {
    Assert-True ($signature.Status -eq "NotSigned") "unexpected signing state: $($signature.Status)"
} else {
    # Project self-signed certs are Authenticode-present but not chain-trusted
    # on the runner (Root import hangs on a confirmation dialog in CI).
    Assert-True (
        $null -ne $signature.SignerCertificate -and
        $signature.Status -in @("Valid", "UnknownError")
    ) "unexpected signing state: $($signature.Status)"
}
Assert-PackageIntegrity $packagePath $installerPath $ExpectedSignature

$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$runRoot = Join-Path $tempRoot "copypaste-windows-evidence-$([guid]::NewGuid())"
$installDir = Join-Path $runRoot "installed"
$dataRoot = Join-Path $runRoot "data"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
$daemon = $null
$app = $null
$daemonOut = Join-Path $runRoot "daemon.stdout.log"
$daemonErr = Join-Path $runRoot "daemon.stderr.log"
$oldDataDir = $env:COPYPASTE_DATA_DIR
$oldSocket = $env:COPYPASTE_SOCKET
$logPath = $null
$failure = $null
$processTrace = $null
$processTracePath = Join-Path $runRoot "process-trace.jsonl"

try {
    $install = Start-Process -FilePath $installerPath -ArgumentList "/S", "/D=$installDir" -Wait -PassThru
    Assert-True ($install.ExitCode -eq 0) "installer exited $($install.ExitCode)"
    Assert-InstalledLayout $installDir
    $processTrace = Start-WindowsProcessTrace $processTracePath

    $ui = Join-Path $installDir "copypaste-ui.exe"
    $cli = Join-Path $installDir "copypaste.exe"
    $daemonExe = Join-Path $installDir "copypaste-daemon.exe"
    $env:COPYPASTE_DATA_DIR = $dataRoot
    $env:COPYPASTE_SOCKET = Join-Path $dataRoot "daemon.sock"
    $daemon = Start-Process -FilePath $daemonExe -ArgumentList "--foreground", "--data-dir", $dataRoot, "--port", "48654", "--device-name", "Windows-CI" -WindowStyle Hidden -RedirectStandardOutput $daemonOut -RedirectStandardError $daemonErr -PassThru
    $status = Wait-Readiness "explicit daemon IPC readiness" {
        if ($daemon.HasExited) { return New-ProbeInvariant "the daemon exited with code $($daemon.ExitCode)" }
        try { return New-ProbeReady (Invoke-Json $cli @("status")) }
        catch { return Get-CliProbeOutcome $_.Exception.Message }
    } { Get-InstalledDiagnostics $null $dataRoot $daemonErr } 15000 20 1
    Assert-True ($status.data.status.clipboard_backend -eq "windows-system-clipboard") "fake clipboard backend"
    Invoke-Json $cli @("add", "named-pipe evidence") | Out-Null
    $search = Invoke-Json $cli @("search", "named-pipe evidence")
    Assert-True ($search.data.page.items.Count -eq 1) "named-pipe add/search did not round-trip"

    Set-Clipboard -Value "native clipboard evidence"
    $captured = Wait-Readiness "native clipboard capture" {
        if ($daemon.HasExited) { return New-ProbeInvariant "the daemon exited with code $($daemon.ExitCode)" }
        try { $reply = Invoke-Json $cli @("search", "native clipboard evidence") }
        catch { return Get-CliProbeOutcome $_.Exception.Message }
        if ($reply.data.page.items.Count -ge 1) { return New-ProbeReady $reply }
        return New-ProbeNotReady "the clipboard text has not reached the store"
    } { Get-InstalledDiagnostics $null $dataRoot $daemonErr } 15000 20 1

    $transfer = Join-Path $runRoot "transfer.json"
    Invoke-Json $cli @("export", "--output", $transfer) | Out-Null
    Assert-True (Test-Path -LiteralPath $transfer -PathType Leaf) "export wrote no file"
    $imported = Invoke-Json $cli @("import", $transfer)
    Assert-True ($imported.data.import.skipped_duplicate -ge 1) "import bypassed duplicate detection"
    Invoke-Json $cli @("shutdown") | Out-Null
    Assert-True ($daemon.WaitForExit(10000)) "daemon ignored shutdown"

    $update = Start-Process -FilePath $installerPath -ArgumentList "/S", "/D=$installDir" -Wait -PassThru
    Assert-True ($update.ExitCode -eq 0) "in-place installer update exited $($update.ExitCode)"
    Assert-InstalledLayout $installDir

    Remove-Item Env:COPYPASTE_DAEMON_BIN -ErrorAction SilentlyContinue
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $app = Start-Process -FilePath $ui -PassThru
    Wait-Readiness "installed app, native window, and installed sidecar readiness" {
        $app.Refresh()
        if ($app.HasExited) { return New-ProbeInvariant "the installed app exited with code $($app.ExitCode)" }
        $root = Get-AppAutomationRoot $app
        if ($null -eq $root) { return New-ProbeNotReady "the app has published no native window handle" }
        $sidecar = Get-InstalledSidecarOutcome $daemonExe $null
        if ($sidecar.kind -ne "ready") { return $sidecar }
        try { Invoke-Json $cli @("status") | Out-Null }
        catch { return Get-CliProbeOutcome $_.Exception.Message }
        return New-ProbeReady $root
    } { Get-InstalledDiagnostics $app $dataRoot $daemonErr } 30000 24 1 | Out-Null
    $preserved = Invoke-Json $cli @("search", "named-pipe evidence")
    Assert-True ($preserved.data.page.items.Count -eq 1) "in-place update lost clipboard history"
    $timer.Stop()
    if ($evidencePath) {
        $featureStates = @()
        $captureTrace = Join-Path $failureDirectory "capture-affinity.jsonl"
        Complete-WindowsFirstRun $app
        Invoke-UiaNamedControl $app "Preferences" "Mode"
        Invoke-UiaNamedControl $app "Privacy & retention" "Allow screenshots"
        Write-WindowCaptureObservation $app $captureTrace "screenshots/before-toggle"
        Set-UiaScreenshots $app $true $captureTrace
        Write-WindowCaptureObservation $app $captureTrace "history/before-navigation"
        Invoke-UiaNamedControl $app "Library" "Clipboard history"
        Write-WindowCaptureObservation $app $captureTrace "history/after-navigation"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "history" "populated" "Clipboard history" "" $captureTrace
        Invoke-UiaNamedControl $app "Connections" "Connect a device"
        $pairingNames = @("Add a CopyPaste device", "Pairing code", "Pairing address", "Pair", "Cancel")
        $pairingVisibleNames = @("Add a CopyPaste device")
        $pairingEnabledNames = @("Pairing code", "Pairing address", "Pair", "Cancel")
        Open-WindowsPairingEntry $app $pairingNames
        $featureStates += Save-WindowsProtectedFeatureState `
            $app $evidencePath "devices" "desktop-pairing-entry" "Pairing code" `
            $pairingNames $pairingVisibleNames $pairingEnabledNames @("Pairing code", "Pairing address") "" $captureTrace
        $restoredShell = Close-WindowsProtectedPairingEntry $app $pairingNames
        Write-WindowCaptureObservation $app $captureTrace "devices/after-close" $restoredShell
        Invoke-UiaNamedControl $app "Preferences" "Mode"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "settings-and-service" "appearance" "Mode" "" $captureTrace
        $updateText = if ($ExpectedSignature -eq "Valid") { "Check for updates" } else { "Updates aren't configured in this build." }
        $updateState = if ($ExpectedSignature -eq "Valid") { "updater-configured" } else { "updater-unconfigured" }
        Invoke-UiaNamedControl $app "About" $updateText
        $featureStates += Save-WindowsFeatureState $app $evidencePath "settings-and-service" $updateState $updateText $updateState $captureTrace
        Invoke-UiaNamedControl $app "Clipboard behavior" "Background capture"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "capture" "service-capture-status" "Background capture" "" $captureTrace
        $featureStates += Save-WindowsFeatureState $app $evidencePath "capture" "copy-feedback-setting" "Copy feedback sound" "copy-feedback-setting" $captureTrace
        Invoke-UiaNamedControl $app "Cloud sync" "Cloud server configuration"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "cloud-account" "unconfigured" "Cloud server configuration" "" $captureTrace
        Write-WindowsFeatureManifest $evidencePath $featureStates
        Invoke-UiaNamedControl $app "Privacy & retention" "Allow screenshots"
        Set-UiaScreenshots $app $false $captureTrace
    }

    Stop-Process -Id $app.Id -Force
    Assert-True ($app.WaitForExit(10000)) "installed Tauri app did not exit"
    $app = $null
    # A killed app must take the sidecar it started with it (ADR-0018's job
    # object). Asking the CLI to shut the daemon down here instead would pass
    # whether or not an orphan was left holding the pipe.
    Wait-Readiness "installed process shutdown" {
        $installedProcesses = @(Get-Process -Name "copypaste-ui", "copypaste", "copypaste-daemon" -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$installDir*" })
        if ($installedProcesses.Count -eq 0) { return New-ProbeReady $true }
        return New-ProbeNotReady "$($installedProcesses.Count) installed process(es) are still running"
    } { Get-InstalledDiagnostics $null $dataRoot $daemonErr } 15000 | Out-Null
    Assert-Unreachable $cli

    $uninstaller = Join-Path $installDir "uninstall.exe"
    $uninstall = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
    Assert-True ($uninstall.ExitCode -eq 0) "uninstaller exited $($uninstall.ExitCode)"
    Assert-True (-not (Test-Path -LiteralPath $ui)) "application survived uninstall"
    Assert-True (-not (Test-Path -LiteralPath $installDir -PathType Container)) "installation directory survived uninstall"

    if ($evidencePath) {
        $logPath = Join-Path $evidencePath "installed-product.log"
        @(
            "installer integrity passed"
            "installed app launched"
            "installed sidecar launched"
            "named-pipe and clipboard passed"
            "update feed contract matched signing mode: $ExpectedSignature"
            "in-place update passed"
            "feature-specific UI states captured"
            "screenshot protection restored"
            "uninstall passed"
        ) | Set-Content -LiteralPath $logPath -Encoding utf8
        $policyScenario = (& python scripts/release/native_evidence_policy.py value --platform windows --field scenario).Trim()
        Assert-True ($LASTEXITCODE -eq 0) "native evidence scenario policy is unavailable"
        $policyBudget = [int]((& python scripts/release/native_evidence_policy.py value --platform windows --field budget_ms).Trim())
        Assert-True ($LASTEXITCODE -eq 0) "native evidence budget policy is unavailable"
        $measurement = @{ platform = "windows"; scenario = $policyScenario; p95_ms = $policyBudget; samples_ms = @($timer.ElapsedMilliseconds) }
        $measurement | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $evidencePath "latency.json") -Encoding utf8
        [string[]]$featureStateArguments = @($featureStates | ForEach-Object {
            "--feature-state"
            "$($_.feature)=$($_.state)"
        })
        python scripts/release/write-native-evidence.py `
            --output (Join-Path $evidencePath "native-evidence.json") `
            --platform windows `
            --environment hosted-runner `
            --os-version ([Environment]::OSVersion.VersionString) `
            --architecture $env:PROCESSOR_ARCHITECTURE `
            --commit $Commit `
            --run-id $RunId `
            --elapsed-ms $timer.ElapsedMilliseconds `
            --qualified-artifact $Installer `
            @featureStateArguments `
            --artifact screenshot=history/screenshot.png `
            --artifact accessibility=history/accessibility.json `
            --artifact screenshot=capture/screenshot.png `
            --artifact accessibility=capture/accessibility.json `
            --artifact accessibility=devices/accessibility.json `
            --artifact screenshot=settings-and-service/screenshot.png `
            --artifact accessibility=settings-and-service/accessibility.json `
            --artifact screenshot=cloud-account/screenshot.png `
            --artifact accessibility=cloud-account/accessibility.json `
            --artifact test-log=installed-product.log `
            --artifact measurement=latency.json `
            --artifact feature-evidence=feature-states.json
        if ($LASTEXITCODE -ne 0) { throw "native evidence writer failed" }
    }

    if ($evidencePath) {
        Remove-Item -LiteralPath (Join-Path $evidencePath "failure-diagnostics") -Recurse -Force
    }
    Write-Output "PASS: $ExpectedSignature current-user install, integrity, installed sidecar, in-place update, update feed contract and uninstall"
} catch {
    # Rethrown after persistence so a diagnostic copy failure cannot replace it.
    $failure = $_
} finally {
    if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id -Force }
    if ($daemon -and -not $daemon.HasExited) { Stop-Process -Id $daemon.Id -Force }
    Invoke-FailureDiagnosticPersistence $failure "process-trace" {
        Stop-WindowsProcessTrace $processTrace
    }
    if ($null -ne $failure -and $evidencePath) {
        Invoke-FailureDiagnosticPersistence $failure "artifact-save" {
            Save-WindowsFailureDiagnostics $dataRoot $processTracePath $evidencePath
        }
    }
    if (Test-Path -LiteralPath (Join-Path $installDir "uninstall.exe")) {
        Start-Process -FilePath (Join-Path $installDir "uninstall.exe") -ArgumentList "/S" -Wait | Out-Null
    }
    if ($null -eq $oldDataDir) { Remove-Item Env:COPYPASTE_DATA_DIR -ErrorAction SilentlyContinue } else { $env:COPYPASTE_DATA_DIR = $oldDataDir }
    if ($null -eq $oldSocket) { Remove-Item Env:COPYPASTE_SOCKET -ErrorAction SilentlyContinue } else { $env:COPYPASTE_SOCKET = $oldSocket }
    Remove-Item -LiteralPath $runRoot -Recurse -Force -ErrorAction SilentlyContinue
}
if ($null -ne $failure) { throw $failure }
