param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$File
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$pfx = $env:WINDOWS_CERTIFICATE_PFX_PATH
$password = $env:WINDOWS_SIGNING_CERTIFICATE_PASSWORD
$timestampUrl = $env:WINDOWS_TIMESTAMP_URL

if ([string]::IsNullOrWhiteSpace($pfx)) { throw "WINDOWS_CERTIFICATE_PFX_PATH is required" }
if (-not (Test-Path -LiteralPath $pfx -PathType Leaf)) { throw "PFX not found at WINDOWS_CERTIFICATE_PFX_PATH" }
if ([string]::IsNullOrWhiteSpace($password)) { throw "WINDOWS_SIGNING_CERTIFICATE_PASSWORD is required" }
if ([string]::IsNullOrWhiteSpace($timestampUrl)) { throw "WINDOWS_TIMESTAMP_URL is required" }
if (-not (Test-Path -LiteralPath $File -PathType Leaf)) { throw "File to sign is missing: $File" }

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq "x64" } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found under Windows Kits" }

# /f + /p bypasses CurrentUser\My private-key CSP issues that break
# Tauri's certificateThumbprint path on hosted runners (alpha.25/26).
& $signtool.FullName sign /fd sha256 /f $pfx /p $password /tr $timestampUrl /td sha256 $File
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
