param(
    [ValidateSet("Prepare", "Validate", "Sign", "Cleanup", "SelfTest")]
    [string]$Operation = "Sign",
    [string]$File,
    [string]$OutputPfxPath,
    [switch]$PersistEnvironment,
    [switch]$SmokeSign
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Require-Environment([string]$Name) {
    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value)) { throw "$Name is required" }
    return $value
}

function Resolve-SignToolTimestampUrl([string]$Raw) {
    if ([string]::IsNullOrWhiteSpace($Raw)) { throw "WINDOWS_TIMESTAMP_URL is required" }
    $trimmed = $Raw.Trim()
    try {
        $uri = [Uri]::new($trimmed)
    } catch {
        throw "WINDOWS_TIMESTAMP_URL is not a valid URI"
    }
    if (-not $uri.IsAbsoluteUri -or $uri.Scheme -notin @("http", "https") -or
        [string]::IsNullOrWhiteSpace($uri.Host)) {
        throw "WINDOWS_TIMESTAMP_URL must be an http(s) URL with a host"
    }

    # SignTool 10.0.26100 rejects https before contacting an RFC 3161 TSA.
    return [regex]::Replace($trimmed, "^https?://", "http://", "IgnoreCase")
}

function Test-CodeSigningUsage(
    [Security.Cryptography.X509Certificates.X509Certificate2]$Certificate
) {
    $eku = $Certificate.Extensions |
        Where-Object { $_.Oid.Value -eq "2.5.29.37" } |
        Select-Object -First 1
    if ($null -eq $eku) { return $true }
    return @($eku.EnhancedKeyUsages | ForEach-Object { $_.Value }) -contains "1.3.6.1.5.5.7.3.3"
}

function Read-SigningCertificate([string]$PfxPath, [string]$Password) {
    if (-not (Test-Path -LiteralPath $PfxPath -PathType Leaf)) {
        throw "WINDOWS_CERTIFICATE_PFX_PATH does not point at a PFX file"
    }

    $collection = [Security.Cryptography.X509Certificates.X509Certificate2Collection]::new()
    try {
        $flags = [Security.Cryptography.X509Certificates.X509KeyStorageFlags]::EphemeralKeySet
        $collection.Import($PfxPath, $Password, $flags)
        $privateKeys = @($collection | Where-Object { $_.HasPrivateKey })
        if ($privateKeys.Count -ne 1) {
            throw "Windows signing PFX must contain exactly one certificate with a private key"
        }
        $certificate = $privateKeys[0]
        if (-not (Test-CodeSigningUsage $certificate)) {
            throw "Windows signing certificate does not permit code signing"
        }
        $now = [DateTime]::UtcNow
        if ($now -lt $certificate.NotBefore.ToUniversalTime() -or
            $now -gt $certificate.NotAfter.ToUniversalTime()) {
            throw "Windows signing certificate is not currently valid"
        }
        foreach ($item in $collection) {
            if (-not [object]::ReferenceEquals($item, $certificate)) { $item.Dispose() }
        }
        return $certificate
    } catch {
        foreach ($item in $collection) { $item.Dispose() }
        throw
    }
}

function Get-PreparedSigningState {
    $pfx = Require-Environment "WINDOWS_CERTIFICATE_PFX_PATH"
    $password = Require-Environment "WINDOWS_SIGNING_CERTIFICATE_PASSWORD"
    $timestampUrl = Require-Environment "WINDOWS_SIGNING_TIMESTAMP_URL"
    if ($timestampUrl -cnotmatch "^http://") {
        throw "WINDOWS_SIGNING_TIMESTAMP_URL was not prepared for SignTool"
    }
    $certificate = Read-SigningCertificate $pfx $password
    return [pscustomobject]@{
        Pfx = $pfx
        Password = $password
        TimestampUrl = $timestampUrl
        Certificate = $certificate
    }
}

function Find-SignTool {
    if (-not [string]::IsNullOrWhiteSpace($env:TAURI_WINDOWS_SIGNTOOL_PATH)) {
        if (-not (Test-Path -LiteralPath $env:TAURI_WINDOWS_SIGNTOOL_PATH -PathType Leaf)) {
            throw "TAURI_WINDOWS_SIGNTOOL_PATH does not point at signtool.exe"
        }
        return $env:TAURI_WINDOWS_SIGNTOOL_PATH
    }
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    $kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    $tool = Get-ChildItem $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.Directory.Name -eq "x64" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if (-not $tool) { throw "signtool.exe not found under Windows Kits" }
    return $tool.FullName
}

function Invoke-WindowsSign([string]$Target) {
    if ($env:OS -ne "Windows_NT") { throw "Authenticode signing must run on Windows" }
    if ([string]::IsNullOrWhiteSpace($Target) -or
        -not (Test-Path -LiteralPath $Target -PathType Leaf)) {
        throw "File to sign is missing"
    }

    $state = $null
    try {
        $state = Get-PreparedSigningState
        $signtool = Find-SignTool
        & $signtool sign /fd sha256 /f $state.Pfx /p $state.Password `
            /tr $state.TimestampUrl /td sha256 $Target
        if ($LASTEXITCODE -ne 0) { throw "signtool sign exited $LASTEXITCODE" }

        $signature = Get-AuthenticodeSignature -LiteralPath $Target
        if ($null -eq $signature.SignerCertificate -or
            $signature.SignerCertificate.Thumbprint -ne $state.Certificate.Thumbprint) {
            throw "Authenticode signer does not match the prepared PFX"
        }
        if ($null -eq $signature.TimeStamperCertificate) {
            throw "Authenticode signature is missing its RFC 3161 timestamp"
        }
    } finally {
        if ($state -and $state.Certificate) { $state.Certificate.Dispose() }
    }
}

function Write-GitHubEnvironment([string]$Name, [string]$Value) {
    if ([string]::IsNullOrWhiteSpace($env:GITHUB_ENV)) {
        throw "GITHUB_ENV is required with -PersistEnvironment"
    }
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "$Name=$Value" -Encoding utf8
}

function Prepare-WindowsSigning {
    $base64 = Require-Environment "WINDOWS_SIGNING_CERTIFICATE_BASE64"
    $password = Require-Environment "WINDOWS_SIGNING_CERTIFICATE_PASSWORD"
    $timestampUrl = Resolve-SignToolTimestampUrl (Require-Environment "WINDOWS_TIMESTAMP_URL")
    if ($PersistEnvironment -and [string]::IsNullOrWhiteSpace($env:GITHUB_ENV)) {
        throw "GITHUB_ENV is required with -PersistEnvironment"
    }
    if ([string]::IsNullOrWhiteSpace($OutputPfxPath)) {
        $temp = Require-Environment "RUNNER_TEMP"
        $script:OutputPfxPath = Join-Path $temp "copypaste-release.pfx"
    }
    $fullPfxPath = [IO.Path]::GetFullPath($OutputPfxPath)
    $bytes = $null
    try {
        $bytes = [Convert]::FromBase64String($base64)
        [IO.File]::WriteAllBytes($fullPfxPath, $bytes)
        $certificate = Read-SigningCertificate $fullPfxPath $password
        $certificate.Dispose()
    } catch {
        Remove-Item -LiteralPath $fullPfxPath -Force -ErrorAction SilentlyContinue
        throw
    } finally {
        if ($null -ne $bytes) { [Array]::Clear($bytes, 0, $bytes.Length) }
    }

    $env:WINDOWS_CERTIFICATE_PFX_PATH = $fullPfxPath
    $env:WINDOWS_SIGNING_TIMESTAMP_URL = $timestampUrl
    if ($PersistEnvironment) {
        Write-GitHubEnvironment "WINDOWS_CERTIFICATE_PFX_PATH" $fullPfxPath
        Write-GitHubEnvironment "WINDOWS_SIGNING_TIMESTAMP_URL" $timestampUrl
    }

    if ($SmokeSign) {
        if ($env:OS -ne "Windows_NT") { throw "-SmokeSign requires Windows" }
        $smoke = Join-Path (Split-Path -Parent $fullPfxPath) "copypaste-sign-smoke.exe"
        try {
            Copy-Item -LiteralPath "$env:SystemRoot\System32\cmd.exe" -Destination $smoke -Force
            Invoke-WindowsSign $smoke
        } finally {
            Remove-Item -LiteralPath $smoke -Force -ErrorAction SilentlyContinue
        }
    }
}

function Invoke-SelfTest {
    $cases = @(
        @{ In = "https://timestamp.digicert.com"; Out = "http://timestamp.digicert.com" },
        @{ In = " HTTP://tsa.example.test/rfc3161 "; Out = "http://tsa.example.test/rfc3161" },
        @{ In = "https://tsa.example.test:8443/path?q=1"; Out = "http://tsa.example.test:8443/path?q=1" }
    )
    foreach ($case in $cases) {
        $actual = Resolve-SignToolTimestampUrl $case.In
        if ($actual -ne $case.Out) { throw "timestamp normalization self-test failed" }
    }

    $old = @{}
    foreach ($name in @(
            "WINDOWS_SIGNING_CERTIFICATE_BASE64",
            "WINDOWS_SIGNING_CERTIFICATE_PASSWORD",
            "WINDOWS_TIMESTAMP_URL",
            "WINDOWS_CERTIFICATE_PFX_PATH",
            "WINDOWS_SIGNING_TIMESTAMP_URL"
        )) {
        $old[$name] = [Environment]::GetEnvironmentVariable($name)
    }
    $testPfx = Join-Path ([IO.Path]::GetTempPath()) "copypaste-signing-self-test-$PID.pfx"
    $rsa = [Security.Cryptography.RSA]::Create(2048)
    $certificate = $null
    try {
        $request = [Security.Cryptography.X509Certificates.CertificateRequest]::new(
            "CN=CopyPaste signing self-test",
            $rsa,
            [Security.Cryptography.HashAlgorithmName]::SHA256,
            [Security.Cryptography.RSASignaturePadding]::Pkcs1
        )
        $usages = [Security.Cryptography.OidCollection]::new()
        [void]$usages.Add([Security.Cryptography.Oid]::new("1.3.6.1.5.5.7.3.3"))
        $request.CertificateExtensions.Add(
            [Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usages, $true)
        )
        $certificate = $request.CreateSelfSigned(
            [DateTimeOffset]::UtcNow.AddMinutes(-1),
            [DateTimeOffset]::UtcNow.AddMinutes(5)
        )
        $password = "self-test-password"
        $env:WINDOWS_SIGNING_CERTIFICATE_BASE64 = [Convert]::ToBase64String(
            $certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $password)
        )
        $env:WINDOWS_SIGNING_CERTIFICATE_PASSWORD = $password
        $env:WINDOWS_TIMESTAMP_URL = "https://timestamp.digicert.com"
        $script:OutputPfxPath = $testPfx
        Prepare-WindowsSigning
        $state = Get-PreparedSigningState
        $state.Certificate.Dispose()
        if ($state.TimestampUrl -ne "http://timestamp.digicert.com") {
            throw "prepared timestamp self-test failed"
        }
        if ($env:OS -eq "Windows_NT") {
            $smoke = Join-Path ([IO.Path]::GetTempPath()) "copypaste-signing-self-test-$PID.exe"
            try {
                Copy-Item -LiteralPath "$env:SystemRoot\System32\cmd.exe" -Destination $smoke -Force
                Invoke-WindowsSign $smoke
            } finally {
                Remove-Item -LiteralPath $smoke -Force -ErrorAction SilentlyContinue
            }
        }
    } finally {
        Remove-Item -LiteralPath $testPfx -Force -ErrorAction SilentlyContinue
        if ($certificate) { $certificate.Dispose() }
        $rsa.Dispose()
        foreach ($name in $old.Keys) {
            [Environment]::SetEnvironmentVariable($name, $old[$name])
        }
    }
    Write-Host "PASS: Windows signing preparation and timestamp contract"
}

switch ($Operation) {
    "Prepare" { Prepare-WindowsSigning }
    "Validate" {
        $state = Get-PreparedSigningState
        $state.Certificate.Dispose()
    }
    "Sign" { Invoke-WindowsSign $File }
    "Cleanup" {
        $pfx = [Environment]::GetEnvironmentVariable("WINDOWS_CERTIFICATE_PFX_PATH")
        if (-not [string]::IsNullOrWhiteSpace($pfx)) {
            Remove-Item -LiteralPath $pfx -Force -ErrorAction SilentlyContinue
        }
        $env:WINDOWS_CERTIFICATE_PFX_PATH = $null
        $env:WINDOWS_SIGNING_TIMESTAMP_URL = $null
    }
    "SelfTest" { Invoke-SelfTest }
}
