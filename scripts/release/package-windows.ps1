param(
    [Parameter(Mandatory = $true)]
    [string]$Installer,
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [ValidateSet("x86_64")]
    [string]$Architecture = "x86_64",
    [string]$UpdaterSignature,
    [string]$ReleaseBaseUrl,
    [string]$PublishDate = ([DateTimeOffset]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ"))
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($Version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$') {
    throw "Version must be a semantic version"
}
if ([bool]$UpdaterSignature -ne [bool]$ReleaseBaseUrl) {
    throw "UpdaterSignature and ReleaseBaseUrl must be supplied together"
}
if ($ReleaseBaseUrl -and -not $ReleaseBaseUrl.StartsWith("https://")) {
    throw "ReleaseBaseUrl must use HTTPS"
}

$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
[IO.Directory]::CreateDirectory($outputPath) | Out-Null
$artifactName = "CopyPaste-v$Version-windows-$Architecture-setup.exe"
$artifactPath = Join-Path $outputPath $artifactName
Copy-Item -LiteralPath $installerPath -Destination $artifactPath -Force

$hashTargets = @($artifactPath)
if ($UpdaterSignature) {
    $signaturePath = (Resolve-Path -LiteralPath $UpdaterSignature).Path
    $signatureName = "$artifactName.sig"
    $packagedSignature = Join-Path $outputPath $signatureName
    Copy-Item -LiteralPath $signaturePath -Destination $packagedSignature -Force
    $signature = (Get-Content -Raw -LiteralPath $signaturePath).Trim()
    if (-not $signature) { throw "Updater signature is empty" }

    $baseUrl = $ReleaseBaseUrl.TrimEnd('/')
    $latest = [ordered]@{
        version = $Version
        pub_date = $PublishDate
        platforms = [ordered]@{
            "windows-$Architecture" = [ordered]@{
                signature = $signature
                url = "$baseUrl/$artifactName"
            }
        }
    }
    $latestPath = Join-Path $outputPath "latest.json"
    $latest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $latestPath -Encoding utf8
    $hashTargets += $packagedSignature, $latestPath
}

$checksumLines = foreach ($path in $hashTargets) {
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($path))"
}
$checksumPath = Join-Path $outputPath "SHA256SUMS"
$checksumLines | Set-Content -LiteralPath $checksumPath -Encoding ascii

[ordered]@{
    artifact = $artifactPath
    checksums = $checksumPath
    sha256 = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
} | ConvertTo-Json -Compress
