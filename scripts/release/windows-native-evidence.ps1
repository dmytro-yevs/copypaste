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

function Write-UiEvidence([Diagnostics.Process]$App, [string]$Directory) {
    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $App.Refresh()
    Assert-True ($App.MainWindowHandle -ne 0) "installed app exposes no native window"

    $bounds = [Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = [Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Location, [Drawing.Point]::Empty, $bounds.Size)
        $bitmap.Save((Join-Path $Directory "screenshot.png"), [Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }

    $nodes = [Windows.Automation.AutomationElement]::RootElement.FindAll(
        [Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.Condition]::TrueCondition
    ) | Where-Object { $_.Current.ProcessId -eq $App.Id } | ForEach-Object {
        [ordered]@{
            name = $_.Current.Name
            control_type = $_.Current.ControlType.ProgrammaticName
            automation_id = $_.Current.AutomationId
        }
    }
    Assert-True (@($nodes).Count -gt 0) "installed app exposes no Windows accessibility nodes"
    @($nodes) | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $Directory "accessibility.json") -Encoding utf8
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
    Start-Sleep -Seconds 3
    if ($daemon.HasExited) {
        $detail = Get-Content -Raw -LiteralPath $daemonErr -ErrorAction SilentlyContinue
        throw "daemon exited before IPC evidence: $detail"
    }

    $status = Invoke-Json $cli @("status")
    Assert-True ($status.data.status.clipboard_backend -eq "windows-system-clipboard") "fake clipboard backend"
    Invoke-Json $cli @("add", "named-pipe evidence") | Out-Null
    $search = Invoke-Json $cli @("search", "named-pipe evidence")
    Assert-True ($search.data.page.items.Count -eq 1) "named-pipe add/search did not round-trip"

    Set-Clipboard -Value "native clipboard evidence"
    Start-Sleep -Seconds 2
    $captured = Invoke-Json $cli @("search", "native clipboard evidence")
    Assert-True ($captured.data.page.items.Count -ge 1) "native clipboard item was not captured"

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
    Start-Sleep -Seconds 5
    Assert-True (-not $app.HasExited) "installed Tauri app did not stay running"
    $sidecars = @(Get-Process -Name "copypaste-daemon" -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $daemonExe })
    if ($sidecars.Count -ne 1) {
        $appLog = Get-ChildItem -LiteralPath (Join-Path $dataRoot "logs") -Filter "*.log" -ErrorAction SilentlyContinue |
            ForEach-Object { Get-Content -Raw -LiteralPath $_.FullName -ErrorAction SilentlyContinue }
        throw "installed UI launched $($sidecars.Count) installed sidecars; app runtime log:`n$($appLog -join "`n")"
    }
    Invoke-Json $cli @("status") | Out-Null
    $preserved = Invoke-Json $cli @("search", "named-pipe evidence")
    Assert-True ($preserved.data.page.items.Count -eq 1) "in-place update lost clipboard history"
    $timer.Stop()
    if ($evidencePath) { Write-UiEvidence $app $evidencePath }

    Stop-Process -Id $app.Id -Force
    Assert-True ($app.WaitForExit(10000)) "installed Tauri app did not exit"
    $app = $null
    Invoke-Json $cli @("shutdown") | Out-Null
    Start-Sleep -Seconds 1
    $installedProcesses = @(Get-Process -Name "copypaste-ui", "copypaste", "copypaste-daemon" -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$installDir*" })
    Assert-True ($installedProcesses.Count -eq 0) "an installed process survived shutdown"

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
            --assertion "uninstall passed" `
            --artifact screenshot=screenshot.png `
            --artifact accessibility=accessibility.json `
            --artifact test-log=installed-product.log `
            --artifact measurement=latency.json
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
