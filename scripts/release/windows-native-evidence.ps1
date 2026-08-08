param(
    [Parameter(Mandatory = $true)]
    [string]$Installer
)

# This script claims only the installed paths it observes. Tray interaction,
# hotkey delivery, toast display and autostart execution need separate UI tests.

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Invoke-Json([string]$Cli, [string[]]$Arguments) {
    $text = & $Cli --json @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw ($text -join "`n") }
    $reply = ($text -join "`n") | ConvertFrom-Json
    Assert-True ($reply.ok -eq $true) "CLI returned a refusal"
    return $reply
}

$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$runRoot = Join-Path $tempRoot "copypaste-windows-evidence-$([guid]::NewGuid())"
$installDir = Join-Path $runRoot "installed"
$dataRoot = Join-Path $runRoot "data"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
$daemon = $null
$app = $null

try {
    $signature = Get-AuthenticodeSignature -FilePath $installerPath
    Assert-True ($signature.Status -eq "NotSigned") "unexpected signing state: $($signature.Status)"

    $install = Start-Process -FilePath $installerPath -ArgumentList "/S", "/D=$installDir" -Wait -PassThru
    Assert-True ($install.ExitCode -eq 0) "installer exited $($install.ExitCode)"

    $ui = Join-Path $installDir "copypaste-ui.exe"
    $cli = Join-Path $installDir "copypaste.exe"
    $daemonExe = Join-Path $installDir "copypaste-daemon.exe"
    foreach ($binary in @($ui, $cli, $daemonExe)) {
        Assert-True (Test-Path -LiteralPath $binary -PathType Leaf) "missing installed binary: $binary"
    }

    $env:APPDATA = $dataRoot
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

    Remove-Item Env:COPYPASTE_DAEMON_BIN -ErrorAction SilentlyContinue
    $app = Start-Process -FilePath $ui -PassThru
    Start-Sleep -Seconds 5
    Assert-True (-not $app.HasExited) "installed Tauri app did not stay running"

    $sidecars = @(Get-Process -Name "copypaste-daemon" -ErrorAction SilentlyContinue | Where-Object {
        $_.Path -eq $daemonExe
    })
    Assert-True ($sidecars.Count -eq 1) "installed UI did not launch its installed sidecar"
    Invoke-Json $cli @("status") | Out-Null

    Stop-Process -Id $app.Id -Force
    Assert-True ($app.WaitForExit(10000)) "installed Tauri app did not exit"
    $app = $null
    Invoke-Json $cli @("shutdown") | Out-Null
    Start-Sleep -Seconds 1
    $installedProcesses = @(Get-Process -Name "copypaste-ui", "copypaste", "copypaste-daemon" -ErrorAction SilentlyContinue | Where-Object {
        $_.Path -like "$installDir*"
    })
    Assert-True ($installedProcesses.Count -eq 0) "an installed process survived shutdown"

    $uninstaller = Join-Path $installDir "uninstall.exe"
    Assert-True (Test-Path -LiteralPath $uninstaller -PathType Leaf) "installer wrote no uninstaller"
    $uninstall = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
    Assert-True ($uninstall.ExitCode -eq 0) "uninstaller exited $($uninstall.ExitCode)"
    Assert-True (-not (Test-Path -LiteralPath $ui)) "application survived uninstall"

    Write-Output "PASS: NotSigned current-user install, installed sidecar lookup, named pipe, clipboard, transfer and uninstall"
}
finally {
    if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id -Force }
    if ($daemon -and -not $daemon.HasExited) { Stop-Process -Id $daemon.Id -Force }
    if (Test-Path -LiteralPath (Join-Path $installDir "uninstall.exe")) {
        Start-Process -FilePath (Join-Path $installDir "uninstall.exe") -ArgumentList "/S" -Wait | Out-Null
    }
    Remove-Item -LiteralPath $runRoot -Recurse -Force -ErrorAction SilentlyContinue
}
