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

function Get-InstalledDiagnostics([Diagnostics.Process]$App, [string]$DataRoot, [string]$DaemonError) {
    $parts = @()
    if ($App) {
        $App.Refresh()
        $parts += if ($App.HasExited) { "app exited with code $($App.ExitCode)" } else { "app is running" }
        try { $parts += Get-UiaSummary $App } catch { $parts += "UIA: $($_.Exception.Message)" }
    }
    if (Test-Path -LiteralPath $DaemonError -PathType Leaf) {
        $parts += "daemon stderr: $((Get-Content -Raw -LiteralPath $DaemonError).Trim())"
    }
    $runtime = Get-ChildItem -LiteralPath (Join-Path $DataRoot "logs") -Filter "*.log" -ErrorAction SilentlyContinue |
        ForEach-Object { Get-Content -Raw -LiteralPath $_.FullName -ErrorAction SilentlyContinue }
    if ($runtime) { $parts += "app runtime log: $($runtime -join "`n")" }
    return $parts
}

function Invoke-SelfTest {
    $root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-windows-evidence-self-test-$([guid]::NewGuid())"
    [IO.Directory]::CreateDirectory($root) | Out-Null
    try {
        foreach ($name in @("copypaste-ui.exe", "copypaste.exe", "uninstall.exe")) {
            [IO.File]::WriteAllText((Join-Path $root $name), "fixture")
        }
        $rejected = $false
        try { Assert-InstalledLayout $root } catch { $rejected = $_.Exception.Message -match "copypaste-daemon.exe" }
        Assert-True $rejected "a package without the installed sidecar did not fail"
        Test-WindowsUiEvidenceHelpers
        Write-Output "PASS: a broken installed sidecar package fails closed"
    } finally {
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}
if (-not $Installer) { throw "Installer is required" }

$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$packagePath = if ($PackageDirectory) {
    (Resolve-Path -LiteralPath $PackageDirectory).Path
} else {
    Split-Path -Parent $installerPath
}
$signature = Get-AuthenticodeSignature -FilePath $installerPath
Assert-True ($signature.Status -eq $ExpectedSignature) "unexpected signing state: $($signature.Status)"
Assert-PackageIntegrity $packagePath $installerPath $ExpectedSignature

$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$runRoot = Join-Path $tempRoot "copypaste-windows-evidence-$([guid]::NewGuid())"
$installDir = Join-Path $runRoot "installed"
$dataRoot = Join-Path $runRoot "data"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
$daemon = $null
$app = $null
$oldDataDir = $env:COPYPASTE_DATA_DIR
$oldSocket = $env:COPYPASTE_SOCKET
$logPath = $null
$evidencePath = $null
if ($EvidenceDirectory) {
    $evidencePath = [IO.Path]::GetFullPath($EvidenceDirectory)
    [IO.Directory]::CreateDirectory($evidencePath) | Out-Null
}

try {
    $install = Start-Process -FilePath $installerPath -ArgumentList "/S", "/D=$installDir" -Wait -PassThru
    Assert-True ($install.ExitCode -eq 0) "installer exited $($install.ExitCode)"
    Assert-InstalledLayout $installDir

    $ui = Join-Path $installDir "copypaste-ui.exe"
    $cli = Join-Path $installDir "copypaste.exe"
    $daemonExe = Join-Path $installDir "copypaste-daemon.exe"
    $env:COPYPASTE_DATA_DIR = $dataRoot
    $env:COPYPASTE_SOCKET = Join-Path $dataRoot "daemon.sock"
    $daemonOut = Join-Path $runRoot "daemon.stdout.log"
    $daemonErr = Join-Path $runRoot "daemon.stderr.log"
    $daemon = Start-Process -FilePath $daemonExe -ArgumentList "--foreground", "--data-dir", $dataRoot, "--port", "48654", "--device-name", "Windows-CI" -WindowStyle Hidden -RedirectStandardOutput $daemonOut -RedirectStandardError $daemonErr -PassThru
    $status = Wait-Observed "explicit daemon IPC readiness" {
        if ($daemon.HasExited) { throw "daemon exited with code $($daemon.ExitCode)" }
        Invoke-Json $cli @("status")
    } { Get-InstalledDiagnostics $null $dataRoot $daemonErr }
    Assert-True ($status.data.status.clipboard_backend -eq "windows-system-clipboard") "fake clipboard backend"
    Invoke-Json $cli @("add", "named-pipe evidence") | Out-Null
    $search = Invoke-Json $cli @("search", "named-pipe evidence")
    Assert-True ($search.data.page.items.Count -eq 1) "named-pipe add/search did not round-trip"

    Set-Clipboard -Value "native clipboard evidence"
    $captured = Wait-Observed "native clipboard capture" {
        $reply = Invoke-Json $cli @("search", "native clipboard evidence")
        if ($reply.data.page.items.Count -ge 1) { return $reply }
        return $null
    } { Get-InstalledDiagnostics $null $dataRoot $daemonErr }

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
    Wait-Observed "installed app, native window, and installed sidecar readiness" {
        $root = Get-AppAutomationRoot $app
        if ($null -eq $root) { return $null }
        $sidecars = @(Get-Process -Name "copypaste-daemon" -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $daemonExe })
        if ($sidecars.Count -ne 1) { return $null }
        Invoke-Json $cli @("status") | Out-Null
        return $root
    } { Get-InstalledDiagnostics $app $dataRoot $daemonErr } 30000 | Out-Null
    $preserved = Invoke-Json $cli @("search", "named-pipe evidence")
    Assert-True ($preserved.data.page.items.Count -eq 1) "in-place update lost clipboard history"
    $timer.Stop()
    if ($evidencePath) {
        $featureStates = @()
        Invoke-UiaNamedControl $app "Settings" "Theme"
        Invoke-UiaNamedControl $app "List" "Allow screenshots"
        Set-UiaScreenshots $app $true
        Invoke-UiaNamedControl $app "History" "Clipboard history"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "history" "populated" "Clipboard history"
        Invoke-UiaNamedControl $app "Devices" "Ready to pair"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "devices" "ready-to-pair" "Ready to pair"
        Invoke-UiaNamedControl $app "Settings" "Theme"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "settings-and-service" "appearance" "Theme"
        $updateText = if ($ExpectedSignature -eq "Valid") { "Check for updates" } else { "Updates aren't configured in this build." }
        $updateState = if ($ExpectedSignature -eq "Valid") { "updater-configured" } else { "updater-unconfigured" }
        Invoke-UiaNamedControl $app "About" $updateText
        $featureStates += Save-WindowsFeatureState $app $evidencePath "settings-and-service" $updateState $updateText $updateState
        Invoke-UiaNamedControl $app "Service" "Background capture"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "capture" "service-capture-status" "Background capture"
        Invoke-UiaNamedControl $app "Sync" "Not configured"
        $featureStates += Save-WindowsFeatureState $app $evidencePath "cloud-account" "not-configured" "Not configured"
        Write-WindowsFeatureManifest $evidencePath $featureStates
        Invoke-UiaNamedControl $app "List" "Allow screenshots"
        Set-UiaScreenshots $app $false
    }

    Stop-Process -Id $app.Id -Force
    Assert-True ($app.WaitForExit(10000)) "installed Tauri app did not exit"
    $app = $null
    Invoke-Json $cli @("shutdown") | Out-Null
    Wait-Observed "installed process shutdown" {
        $installedProcesses = @(Get-Process -Name "copypaste-ui", "copypaste", "copypaste-daemon" -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$installDir*" })
        if ($installedProcesses.Count -eq 0) { return $true }
        return $null
    } { Get-InstalledDiagnostics $null $dataRoot $daemonErr } | Out-Null

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
        $measurement = @{ platform = "windows"; scenario = "installed-sidecar-ready"; p95_ms = 30000; samples_ms = @($timer.ElapsedMilliseconds) }
        $measurement | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $evidencePath "latency.json") -Encoding utf8
        python scripts/release/write-native-evidence.py `
            --output (Join-Path $evidencePath "native-evidence.json") `
            --platform windows `
            --environment hosted-runner `
            --os-version ([Environment]::OSVersion.VersionString) `
            --architecture $env:PROCESSOR_ARCHITECTURE `
            --commit $Commit `
            --run-id $RunId `
            --scenario windows-installed-release `
            --elapsed-ms $timer.ElapsedMilliseconds `
            --budget-ms 30000 `
            --assertion "installer integrity passed" `
            --assertion "installed app launched" `
            --assertion "installed sidecar launched" `
            --assertion "named-pipe and clipboard passed" `
            --assertion "update feed contract matched signing mode" `
            --assertion "in-place update passed" `
            --assertion "feature-specific UI states captured" `
            --assertion "screenshot protection restored" `
            --assertion "uninstall passed" `
            --artifact screenshot=history/screenshot.png `
            --artifact accessibility=history/accessibility.json `
            --artifact screenshot=capture/screenshot.png `
            --artifact accessibility=capture/accessibility.json `
            --artifact screenshot=devices/screenshot.png `
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

    Write-Output "PASS: $ExpectedSignature current-user install, integrity, installed sidecar, in-place update, update feed contract and uninstall"
} finally {
    if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id -Force }
    if ($daemon -and -not $daemon.HasExited) { Stop-Process -Id $daemon.Id -Force }
    if (Test-Path -LiteralPath (Join-Path $installDir "uninstall.exe")) {
        Start-Process -FilePath (Join-Path $installDir "uninstall.exe") -ArgumentList "/S" -Wait | Out-Null
    }
    if ($null -eq $oldDataDir) { Remove-Item Env:COPYPASTE_DATA_DIR -ErrorAction SilentlyContinue } else { $env:COPYPASTE_DATA_DIR = $oldDataDir }
    if ($null -eq $oldSocket) { Remove-Item Env:COPYPASTE_SOCKET -ErrorAction SilentlyContinue } else { $env:COPYPASTE_SOCKET = $oldSocket }
    Remove-Item -LiteralPath $runRoot -Recurse -Force -ErrorAction SilentlyContinue
}
