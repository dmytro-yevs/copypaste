param(
    [Parameter(Mandatory = $true)]
    [string]$Installer,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+$')]
    [string]$Version
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
if ($env:OS -ne "Windows_NT") { throw "Windows smoke requires Windows" }
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$installDirectory = Join-Path $env:LOCALAPPDATA "Programs\CopyPaste"
$app = Join-Path $installDirectory "CopyPaste.exe"
$uninstaller = Join-Path $installDirectory "Uninstall.exe"

try {
    $install = Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait -PassThru
    if ($install.ExitCode -ne 0) { throw "installer exited with $($install.ExitCode)" }
    if (-not (Test-Path -LiteralPath $app -PathType Leaf)) { throw "installed application is missing" }
    $versionInfo = [Diagnostics.FileVersionInfo]::GetVersionInfo($app)
    $productVersion = "$($versionInfo.ProductMajorPart).$($versionInfo.ProductMinorPart).$($versionInfo.ProductBuildPart)"
    if ($productVersion -cne $Version) {
        throw "installed application version $productVersion does not match $Version"
    }

    $process = Start-Process -FilePath $app -PassThru
    $daemon = $null
    for ($attempt = 0; $attempt -lt 60 -and $null -eq $daemon; $attempt++) {
        Start-Sleep -Milliseconds 500
        if ($process.HasExited) { throw "installed application exited during startup" }
        $daemon = Get-CimInstance Win32_Process -Filter "Name = 'copypaste-daemon.exe'" |
            Where-Object { $_.ParentProcessId -eq $process.Id } |
            Select-Object -First 1
    }
    if ($null -eq $daemon) { throw "installed application did not start its bundled daemon" }

    Stop-Process -Id $process.Id
    $process.WaitForExit(10000) | Out-Null
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        $alive = Get-Process -Id $daemon.ProcessId -ErrorAction SilentlyContinue
        if ($null -eq $alive) { break }
        Start-Sleep -Milliseconds 250
    }
    if (Get-Process -Id $daemon.ProcessId -ErrorAction SilentlyContinue) {
        throw "bundled daemon survived application exit"
    }
} finally {
    Get-Process CopyPaste -ErrorAction SilentlyContinue | Stop-Process -Force
    Get-Process copypaste-daemon -ErrorAction SilentlyContinue | Stop-Process -Force
    if (Test-Path -LiteralPath $uninstaller -PathType Leaf) {
        Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait | Out-Null
    }
}

Write-Host "verified Windows installer and app-owned daemon lifecycle"
