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

function Get-SignToolArguments([object]$State, [string]$Target) {
    return @(
        "sign", "/fd", "sha256", "/f", $State.Pfx, "/p", $State.Password,
        "/tr", $State.TimestampUrl, "/td", "sha256", $Target
    )
}

function Invoke-BoundedProcess(
    [string]$Executable,
    [string[]]$Arguments,
    [int]$TimeoutMilliseconds,
    [string]$Phase,
    [string]$Target
) {
    if ($TimeoutMilliseconds -le 0) { throw "Process timeout must be positive" }
    $start = New-InheritedProcessStartInfo $Executable $Arguments

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    try {
        try {
            if (-not $process.Start()) { throw "process did not start" }
        } catch {
            throw (Format-ProcessDiagnostic $Phase $Executable $Target "start failed")
        }
        if (-not $process.WaitForExit($TimeoutMilliseconds)) {
            $killFailed = $false
            try { $process.Kill($true) } catch { $killFailed = $true }
            $terminated = $process.WaitForExit(5000)
            if (-not $terminated) {
                throw (Format-ProcessDiagnostic $Phase $Executable $Target `
                    "timed out after $TimeoutMilliseconds ms; termination unconfirmed")
            }
            if ($killFailed) {
                throw (Format-ProcessDiagnostic $Phase $Executable $Target `
                    "timed out after $TimeoutMilliseconds ms; process-tree kill failed")
            }
            throw (Format-ProcessDiagnostic $Phase $Executable $Target `
                "timed out after $TimeoutMilliseconds ms")
        }
        return [pscustomobject]@{ ExitCode = $process.ExitCode }
    } finally {
        $process.Dispose()
    }
}

function New-InheritedProcessStartInfo([string]$Executable, [string[]]$Arguments) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    foreach ($argument in $Arguments) { [void]$start.ArgumentList.Add($argument) }
    return $start
}

function Format-ProcessDiagnostic(
    [string]$Phase,
    [string]$Executable,
    [string]$Target,
    [string]$Status
) {
    $executableName = [IO.Path]::GetFileName($Executable)
    $targetName = [IO.Path]::GetFileName($Target)
    return "$Phase failed (executable=$executableName, target=$targetName, status=$Status)"
}

function Normalize-CertificateThumbprint([string]$Thumbprint) {
    if ([string]::IsNullOrWhiteSpace($Thumbprint)) {
        throw "Certificate thumbprint is missing"
    }
    $normalized = [regex]::Replace($Thumbprint, "[\s:]", "").ToUpperInvariant()
    if ($normalized -cnotmatch "^[0-9A-F]{40}$") {
        throw "Certificate thumbprint is not a SHA-1 hexadecimal value"
    }
    return $normalized
}

function Assert-AuthenticodeSigner(
    [object]$Signature,
    [Security.Cryptography.X509Certificates.X509Certificate2]$ExpectedCertificate
) {
    if ($Signature.Status -ne "Valid") {
        throw "Authenticode signature status is not valid"
    }
    if ($Signature.SignatureType -ne "Authenticode") {
        throw "Signed file did not expose an embedded Authenticode signature"
    }
    if ($null -eq $Signature.SignerCertificate) {
        throw "Authenticode signer does not match the prepared PFX"
    }
    $actualThumbprint = Normalize-CertificateThumbprint $Signature.SignerCertificate.Thumbprint
    $expectedThumbprint = Normalize-CertificateThumbprint $ExpectedCertificate.Thumbprint
    if ($actualThumbprint -cne $expectedThumbprint) {
        throw "Authenticode signer does not match the prepared PFX"
    }
}

# 16 MiB is well above normal Authenticode chains while bounding hostile PE allocation.
$script:MaximumEmbeddedSignatureBytes = 16MB

function Read-Exactly([IO.Stream]$Stream, [byte[]]$Buffer) {
    $offset = 0
    while ($offset -lt $Buffer.Length) {
        $read = $Stream.Read($Buffer, $offset, $Buffer.Length - $offset)
        if ($read -eq 0) { throw "Embedded Authenticode signature is truncated" }
        $offset += $read
    }
}

function Read-EmbeddedSignatureCms([string]$Target) {
    Add-Type -AssemblyName System.Reflection.Metadata
    Add-Type -AssemblyName System.Security.Cryptography.Pkcs
    Add-Type -AssemblyName System.Formats.Asn1
    $stream = [IO.File]::OpenRead($Target)
    try {
        $reader = [Reflection.PortableExecutable.PEReader]::new($stream)
        try {
            $directory = $reader.PEHeaders.PEHeader.CertificateTableDirectory
            if ($directory.RelativeVirtualAddress -le 0 -or $directory.Size -lt 8) {
                throw "Signed file did not expose an embedded Authenticode signature"
            }
            if ($directory.Size -gt $script:MaximumEmbeddedSignatureBytes -or
                [int64]$directory.RelativeVirtualAddress + [int64]$directory.Size -gt $stream.Length) {
                throw "Embedded Authenticode signature has an invalid size"
            }
            $stream.Position = $directory.RelativeVirtualAddress
            $header = [byte[]]::new(8)
            Read-Exactly $stream $header
            $length = [BitConverter]::ToUInt32($header, 0)
            $certificateType = [BitConverter]::ToUInt16($header, 6)
            if ($length -lt 8 -or $length -gt $directory.Size -or
                $length -gt $script:MaximumEmbeddedSignatureBytes -or $certificateType -ne 2) {
                throw "Signed file did not expose an embedded Authenticode signature"
            }
            $payloadLength = [int]($length - 8)
            $encoded = [byte[]]::new($payloadLength)
            Read-Exactly $stream $encoded
            $cms = [Security.Cryptography.Pkcs.SignedCms]::new()
            $cms.Decode($encoded)
            return $cms
        } finally {
            $reader.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
}

function Assert-Sha256AlgorithmIdentifier([Formats.Asn1.AsnReader]$Reader, [string]$Error) {
    $algorithm = $Reader.ReadSequence()
    if ($algorithm.ReadObjectIdentifier() -cne "2.16.840.1.101.3.4.2.1") { throw $Error }
    if ($algorithm.HasData) { $algorithm.ReadNull() }
    if ($algorithm.HasData) { throw "Embedded Authenticode signature contains malformed ASN.1" }
}

function Get-Rfc3161TimestampCertificate(
    [Security.Cryptography.Pkcs.SignerInfo]$Signer
) {
    $timestampAttribute = $Signer.UnsignedAttributes |
        Where-Object { $_.Oid.Value -eq "1.3.6.1.4.1.311.3.3.1" } |
        Select-Object -First 1
    if ($null -eq $timestampAttribute -or $timestampAttribute.Values.Count -lt 1) { return $null }

    $timestampCms = [Security.Cryptography.Pkcs.SignedCms]::new()
    $timestampCms.Decode($timestampAttribute.Values[0].RawData)
    if ($timestampCms.ContentInfo.ContentType.Value -cne "1.2.840.113549.1.9.16.1.4" -or
        $timestampCms.SignerInfos.Count -lt 1) {
        throw "Authenticode RFC 3161 timestamp is invalid"
    }
    if ($timestampCms.SignerInfos[0].DigestAlgorithm.Value -cne "2.16.840.1.101.3.4.2.1") {
        throw "Authenticode RFC 3161 timestamp signature does not use SHA-256"
    }
    $tstInfoReader = [Formats.Asn1.AsnReader]::new(
        $timestampCms.ContentInfo.Content, [Formats.Asn1.AsnEncodingRules]::DER)
    $tstInfo = $tstInfoReader.ReadSequence()
    [void]$tstInfo.ReadInteger()
    [void]$tstInfo.ReadObjectIdentifier()
    $messageImprint = $tstInfo.ReadSequence()
    Assert-Sha256AlgorithmIdentifier $messageImprint `
        "Authenticode RFC 3161 message imprint does not use SHA-256"
    [void]$messageImprint.ReadOctetString()
    if ($messageImprint.HasData) {
        throw "Authenticode RFC 3161 timestamp is invalid"
    }
    [void]$tstInfo.ReadInteger()
    [void]$tstInfo.ReadGeneralizedTime()
    while ($tstInfo.HasData) { [void]$tstInfo.ReadEncodedValue() }
    if ($tstInfoReader.HasData) { throw "Authenticode RFC 3161 timestamp is invalid" }
    return $timestampCms.SignerInfos[0].Certificate
}

function New-SelfTestCms(
    [Security.Cryptography.X509Certificates.X509Certificate2]$Certificate,
    [string]$SignerDigestOid,
    [string]$ImprintDigestOid,
    [switch]$Timestamp,
    [switch]$InvalidTimestamp,
    [string]$TimestampSignerDigestOid = "2.16.840.1.101.3.4.2.1"
) {
    $indirectWriter = [Formats.Asn1.AsnWriter]::new([Formats.Asn1.AsnEncodingRules]::DER)
    $indirectWriter.PushSequence()
    $indirectWriter.PushSequence()
    $indirectWriter.WriteObjectIdentifier("1.3.6.1.4.1.311.2.1.15")
    $indirectWriter.PopSequence()
    $indirectWriter.PushSequence()
    $indirectWriter.PushSequence()
    $indirectWriter.WriteObjectIdentifier("2.16.840.1.101.3.4.2.1")
    $indirectWriter.WriteNull()
    $indirectWriter.PopSequence()
    $indirectWriter.WriteOctetString([byte[]]::new(32))
    $indirectWriter.PopSequence()
    $indirectWriter.PopSequence()
    $content = [Security.Cryptography.Pkcs.ContentInfo]::new(
        [Security.Cryptography.Oid]::new("1.3.6.1.4.1.311.2.1.4"), $indirectWriter.Encode())
    $cms = [Security.Cryptography.Pkcs.SignedCms]::new($content, $false)
    $cmsSigner = [Security.Cryptography.Pkcs.CmsSigner]::new($Certificate)
    $cmsSigner.DigestAlgorithm = [Security.Cryptography.Oid]::new($SignerDigestOid)
    $cms.ComputeSignature($cmsSigner)
    if (-not $Timestamp) { return $cms }

    $tstWriter = [Formats.Asn1.AsnWriter]::new([Formats.Asn1.AsnEncodingRules]::DER)
    $tstWriter.PushSequence()
    $tstWriter.WriteInteger(1)
    $tstWriter.WriteObjectIdentifier("1.2.3.4")
    $tstWriter.PushSequence()
    $tstWriter.PushSequence()
    $tstWriter.WriteObjectIdentifier($ImprintDigestOid)
    $tstWriter.WriteNull()
    $tstWriter.PopSequence()
    $tstWriter.WriteOctetString([byte[]]::new(32))
    $tstWriter.PopSequence()
    $tstWriter.WriteInteger(1)
    $tstWriter.WriteGeneralizedTime([DateTimeOffset]::UtcNow)
    $tstWriter.PopSequence()
    $tstBytes = $tstWriter.Encode()
    $tstContentOid = if ($InvalidTimestamp) {
        "1.2.840.113549.1.7.1"
    } else {
        "1.2.840.113549.1.9.16.1.4"
    }
    $tstContent = [Security.Cryptography.Pkcs.ContentInfo]::new(
        [Security.Cryptography.Oid]::new($tstContentOid), $tstBytes)
    $timestampCms = [Security.Cryptography.Pkcs.SignedCms]::new($tstContent, $false)
    $timestampSigner = [Security.Cryptography.Pkcs.CmsSigner]::new($Certificate)
    $timestampSigner.DigestAlgorithm = [Security.Cryptography.Oid]::new($TimestampSignerDigestOid)
    $timestampCms.ComputeSignature($timestampSigner)
    $attributeOid = [Security.Cryptography.Oid]::new("1.3.6.1.4.1.311.3.3.1")
    $attribute = [Security.Cryptography.AsnEncodedData]::new(
        $attributeOid, $timestampCms.Encode())
    $cms.SignerInfos[0].AddUnsignedAttribute($attribute)
    return $cms
}

function Convert-EmbeddedSignatureCms([Security.Cryptography.Pkcs.SignedCms]$Cms) {
    if ($Cms.SignerInfos.Count -lt 1) {
        throw "Signed file did not expose an embedded Authenticode signature"
    }
    $signer = $Cms.SignerInfos[0]
    $authenticode = [Formats.Asn1.AsnReader]::new(
        $Cms.ContentInfo.Content, [Formats.Asn1.AsnEncodingRules]::DER)
    $indirectData = $authenticode.ReadSequence()
    [void]$indirectData.ReadEncodedValue()
    $digestInfo = $indirectData.ReadSequence()
    Assert-Sha256AlgorithmIdentifier $digestInfo "Authenticode file digest does not use SHA-256"
    [void]$digestInfo.ReadOctetString()
    if ($digestInfo.HasData -or $indirectData.HasData -or $authenticode.HasData) {
        throw "Embedded Authenticode signature contains malformed ASN.1"
    }
    if ($signer.DigestAlgorithm.Value -cne "2.16.840.1.101.3.4.2.1") {
        throw "Authenticode signature does not use SHA-256"
    }
    return [pscustomobject]@{
        Status = "Valid"
        SignatureType = "Authenticode"
        SignerCertificate = $signer.Certificate
        TimeStamperCertificate = Get-Rfc3161TimestampCertificate $signer
    }
}

function Get-EmbeddedAuthenticodeSignature(
    [string]$Target,
    [string]$SignTool,
    [scriptblock]$ProcessRunner,
    [scriptblock]$CmsReader = { param($Path) Read-EmbeddedSignatureCms $Path }
) {
    $verification = & $ProcessRunner $SignTool @("verify", "/pa", "/all", "/tw", $Target) `
        120000 "SignTool embedded verification" $Target
    if ($verification.ExitCode -ne 0) {
        throw (Format-ProcessDiagnostic "SignTool embedded verification" $SignTool $Target `
            "exit $($verification.ExitCode)")
    }

    return Convert-EmbeddedSignatureCms (& $CmsReader $Target)
}

function Invoke-WindowsSign(
    [string]$Target,
    [scriptblock]$ProcessRunner = { param($Exe, $RunnerArguments, $Timeout, $Phase, $RunnerTarget)
        Invoke-BoundedProcess $Exe $RunnerArguments $Timeout $Phase $RunnerTarget
    },
    [scriptblock]$SigningStateReader = { Get-PreparedSigningState },
    [scriptblock]$SignToolFinder = { Find-SignTool },
    [scriptblock]$SignatureReader = {
        param($Path, $Tool, $Runner) Get-EmbeddedAuthenticodeSignature $Path $Tool $Runner
    }
) {
    if ($env:OS -ne "Windows_NT") { throw "Authenticode signing must run on Windows" }
    if ([string]::IsNullOrWhiteSpace($Target) -or
        -not (Test-Path -LiteralPath $Target -PathType Leaf)) {
        throw "File to sign is missing"
    }

    $state = $null
    try {
        $state = & $SigningStateReader
        $signtool = & $SignToolFinder
        $arguments = Get-SignToolArguments $state $Target
        $result = & $ProcessRunner $signtool $arguments 120000 "SignTool signing" $Target
        if ($result.ExitCode -ne 0) {
            throw (Format-ProcessDiagnostic "SignTool signing" $signtool $Target `
                "exit $($result.ExitCode)")
        }

        $signature = & $SignatureReader $Target $signtool $ProcessRunner
        Assert-AuthenticodeSigner $signature $state.Certificate
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
    Add-Type -AssemblyName System.Security.Cryptography.Pkcs
    Add-Type -AssemblyName System.Formats.Asn1
    Write-Host "PHASE: Windows signing contracts"
    $cases = @(
        @{ In = "https://timestamp.digicert.com"; Out = "http://timestamp.digicert.com" },
        @{ In = " HTTP://tsa.example.test/rfc3161 "; Out = "http://tsa.example.test/rfc3161" },
        @{ In = "https://tsa.example.test:8443/path?q=1"; Out = "http://tsa.example.test:8443/path?q=1" }
    )
    foreach ($case in $cases) {
        $actual = Resolve-SignToolTimestampUrl $case.In
        if ($actual -ne $case.Out) { throw "timestamp normalization self-test failed" }
    }

    $argumentState = [pscustomobject]@{
        Pfx = "prepared.pfx"
        Password = "prepared-password"
        TimestampUrl = "http://tsa.example.test/rfc3161"
    }
    $target = "target with spaces.exe"
    $arguments = @(Get-SignToolArguments $argumentState $target)
    $expectedArguments = @(
        "sign", "/fd", "sha256", "/f", "prepared.pfx", "/p", "prepared-password",
        "/tr", "http://tsa.example.test/rfc3161", "/td", "sha256", $target
    )
    if ([string]::Join("`n", $arguments) -cne [string]::Join("`n", $expectedArguments)) {
        throw "SignTool argument contract self-test failed"
    }

    Write-Host "PHASE: bounded external processes"
    $pwsh = (Get-Process -Id $PID).Path
    $routing = New-InheritedProcessStartInfo $pwsh @("-NoProfile", "-Command", "exit 0")
    if ($routing.RedirectStandardOutput -or $routing.RedirectStandardError -or
        $routing.UseShellExecute -or -not $routing.CreateNoWindow) {
        throw "bounded process output routing self-test failed"
    }
    $failed = Invoke-BoundedProcess $pwsh @("-NoProfile", "-Command", "exit 23") `
        10000 "bounded runner self-test" "failure-probe"
    if ($failed.ExitCode -ne 23) { throw "bounded process failure self-test failed" }
    $timedOut = $false
    try {
        Invoke-BoundedProcess $pwsh @("-NoProfile", "-Command", `
            "Start-Sleep 30") `
            250 "bounded runner self-test" "timeout-probe"
    } catch {
        $message = $_.Exception.Message
        if ($message -cnotmatch "^bounded runner self-test failed " -or
            $message -cnotmatch "executable=.*pwsh.*target=timeout-probe" -or
            $message -cnotmatch "status=timed out after 250 ms\)$") { throw }
        $timedOut = $true
    }
    if (-not $timedOut) { throw "bounded process timeout self-test failed" }

    Write-Host "PHASE: certificate preparation and signer validation"
    $old = @{}
    foreach ($name in @(
            "WINDOWS_SIGNING_CERTIFICATE_BASE64",
            "WINDOWS_SIGNING_CERTIFICATE_PASSWORD",
            "WINDOWS_TIMESTAMP_URL",
            "WINDOWS_CERTIFICATE_PFX_PATH",
            "WINDOWS_SIGNING_TIMESTAMP_URL",
            "GITHUB_ENV",
            "OS"
        )) {
        $old[$name] = [Environment]::GetEnvironmentVariable($name)
    }
    $testPfx = Join-Path ([IO.Path]::GetTempPath()) "copypaste-signing-self-test-$PID.pfx"
    $testEnvironment = "$testPfx.env"
    $catalogTarget = "$testPfx.catalog.exe"
    $sha1Target = "$testPfx.sha1.exe"
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
        Write-Host "PHASE: offline embedded-signature policy"
        $acceptVerification = {
            param($Exe, $RunnerArguments, $Timeout, $Phase, $RunnerTarget)
            [pscustomobject]@{ ExitCode = 0 }
        }
        $rejectVerification = {
            param($Exe, $RunnerArguments, $Timeout, $Phase, $RunnerTarget)
            [pscustomobject]@{ ExitCode = 17 }
        }
        $verifyRejected = $false
        try {
            Get-EmbeddedAuthenticodeSignature "fixture.exe" "signtool.exe" `
                $rejectVerification { throw "CMS reader must not run after verify failure" }
        } catch {
            if ($_.Exception.Message -cnotmatch "status=exit 17") { throw }
            $verifyRejected = $true
        }
        if (-not $verifyRejected) { throw "SignTool verify failure self-test failed" }

        $policyCases = @(
            @{
                Name = "missing timestamp"
                Cms = New-SelfTestCms $certificate "2.16.840.1.101.3.4.2.1" `
                    "2.16.840.1.101.3.4.2.1"
                Error = $null
            },
            @{
                Name = "invalid timestamp"
                Cms = New-SelfTestCms $certificate "2.16.840.1.101.3.4.2.1" `
                    "2.16.840.1.101.3.4.2.1" -Timestamp -InvalidTimestamp
                Error = "Authenticode RFC 3161 timestamp is invalid"
            },
            @{
                Name = "SHA-1 signer digest"
                Cms = New-SelfTestCms $certificate "1.3.14.3.2.26" `
                    "2.16.840.1.101.3.4.2.1" -Timestamp
                Error = "Authenticode signature does not use SHA-256"
            },
            @{
                Name = "SHA-1 timestamp signer digest"
                Cms = New-SelfTestCms $certificate "2.16.840.1.101.3.4.2.1" `
                    "2.16.840.1.101.3.4.2.1" -Timestamp -TimestampSignerDigestOid "1.3.14.3.2.26"
                Error = "Authenticode RFC 3161 timestamp signature does not use SHA-256"
            },
            @{
                Name = "SHA-1 timestamp imprint"
                Cms = New-SelfTestCms $certificate "2.16.840.1.101.3.4.2.1" `
                    "1.3.14.3.2.26" -Timestamp
                Error = "Authenticode RFC 3161 message imprint does not use SHA-256"
            },
            @{
                Name = "SHA-256 timestamp imprint"
                Cms = New-SelfTestCms $certificate "2.16.840.1.101.3.4.2.1" `
                    "2.16.840.1.101.3.4.2.1" -Timestamp
                Error = $null
            }
        )
        foreach ($case in $policyCases) {
            $cmsReader = { $case.Cms }.GetNewClosure()
            $errorMessage = $null
            try {
                $validated = Get-EmbeddedAuthenticodeSignature "fixture.exe" "signtool.exe" `
                    $acceptVerification $cmsReader
            } catch {
                $errorMessage = $_.Exception.Message
            }
            if ($case.Name -eq "missing timestamp") {
                if ($errorMessage -or $null -ne $validated.TimeStamperCertificate) {
                    throw "missing timestamp fixture self-test failed"
                }
            } elseif ($errorMessage -cne $case.Error) {
                throw "$($case.Name) fixture self-test failed: $errorMessage"
            } elseif ($null -eq $case.Error -and $null -eq $validated.TimeStamperCertificate) {
                throw "$($case.Name) fixture self-test failed"
            }
        }
        $password = "self-test-password"
        [IO.File]::WriteAllBytes($testPfx, $certificate.Export(
                [Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $password))
        if ($env:OS -eq "Windows_NT") {
            $signtool = Find-SignTool
            Copy-Item -LiteralPath "$env:SystemRoot\System32\cmd.exe" -Destination $catalogTarget
            $catalogBefore = Get-AuthenticodeSignature -LiteralPath $catalogTarget
            if ($catalogBefore.SignatureType -ne "Catalog") {
                throw "catalog precedence self-test source is not catalog signed"
            }
            $signResult = Invoke-BoundedProcess $signtool @(
                "sign", "/fd", "sha256", "/f", $testPfx, "/p", $password, $catalogTarget
            ) 120000 "embedded signature self-test" $catalogTarget
            if ($signResult.ExitCode -ne 0) { throw "embedded signature self-test signing failed" }
            $catalogAfter = Get-AuthenticodeSignature -LiteralPath $catalogTarget
            if ($catalogAfter.SignatureType -ne "Catalog") {
                throw "catalog precedence self-test did not reproduce catalog selection"
            }
            $verificationCalls = [Collections.Generic.List[object]]::new()
            $acceptVerification = {
                param($Exe, $RunnerArguments, $Timeout, $Phase, $RunnerTarget)
                $verificationCalls.Add([pscustomobject]@{
                    Executable = $Exe; Arguments = @($RunnerArguments); Timeout = $Timeout
                    Phase = $Phase; Target = $RunnerTarget
                })
                [pscustomobject]@{ ExitCode = 0 }
            }.GetNewClosure()
            $embedded = Get-EmbeddedAuthenticodeSignature $catalogTarget $signtool $acceptVerification
            Assert-AuthenticodeSigner $embedded $certificate
            if ($null -ne $embedded.TimeStamperCertificate) {
                throw "untimestamped embedded signature self-test exposed a timestamp"
            }
            if ($verificationCalls.Count -ne 1 -or
                [string]::Join(" ", $verificationCalls[0].Arguments) -cne
                    "verify /pa /all /tw $catalogTarget" -or
                $verificationCalls[0].Timeout -ne 120000) {
                throw "embedded verification mode self-test failed"
            }

            Copy-Item -LiteralPath "$env:SystemRoot\System32\cmd.exe" -Destination $sha1Target
            $sha1Result = Invoke-BoundedProcess $signtool @(
                "sign", "/fd", "sha1", "/f", $testPfx, "/p", $password, $sha1Target
            ) 120000 "SHA-1 rejection self-test" $sha1Target
            if ($sha1Result.ExitCode -ne 0) { throw "SHA-1 rejection self-test signing failed" }
            $sha1Rejected = $false
            try {
                Get-EmbeddedAuthenticodeSignature $sha1Target $signtool $acceptVerification
            } catch {
                if ($_.Exception.Message -cne "Authenticode file digest does not use SHA-256") { throw }
                $sha1Rejected = $true
            }
            if (-not $sha1Rejected) { throw "SHA-1 embedded signature self-test failed" }
        }
        $otherCertificate = $request.CreateSelfSigned(
            [DateTimeOffset]::UtcNow.AddMinutes(-1),
            [DateTimeOffset]::UtcNow.AddMinutes(4)
        )
        try {
            $normalized = Normalize-CertificateThumbprint (
                "  " + ($certificate.Thumbprint -replace "(.{2})", '$1:') + "  "
            )
            if ($normalized -cne $certificate.Thumbprint.ToUpperInvariant()) {
                throw "certificate thumbprint normalization self-test failed"
            }

            $invalidStatuses = @(
                "UnknownError",
                "NotSigned",
                "HashMismatch",
                "NotTrusted",
                "NotSupportedFileFormat",
                "Incompatible"
            )
            foreach ($status in $invalidStatuses) {
                $invalidSignature = [pscustomobject]@{
                    Status = $status
                    SignatureType = "Authenticode"
                    SignerCertificate = $certificate
                }
                $rejected = $false
                try {
                    Assert-AuthenticodeSigner $invalidSignature $certificate
                } catch {
                    if ($_.Exception.Message -cne "Authenticode signature status is not valid") {
                        throw
                    }
                    $rejected = $true
                }
                if (-not $rejected) { throw "$status signature status self-test failed" }
            }

            $identityCases = @(
                @{
                    Name = "missing signer"
                    Error = "Authenticode signer does not match the prepared PFX"
                    Signature = [pscustomobject]@{
                        Status = "Valid"
                        SignatureType = "Authenticode"
                        SignerCertificate = $null
                    }
                },
                @{
                    Name = "non-Authenticode signature"
                    Error = "Signed file did not expose an embedded Authenticode signature"
                    Signature = [pscustomobject]@{
                        Status = "Valid"
                        SignatureType = "Catalog"
                        SignerCertificate = $certificate
                    }
                },
                @{
                    Name = "malformed signer thumbprint"
                    Error = "Certificate thumbprint is not a SHA-1 hexadecimal value"
                    Signature = [pscustomobject]@{
                        Status = "Valid"
                        SignatureType = "Authenticode"
                        SignerCertificate = [pscustomobject]@{ Thumbprint = "not-a-thumbprint" }
                    }
                },
                @{
                    Name = "mismatched signer thumbprint"
                    Error = "Authenticode signer does not match the prepared PFX"
                    Signature = [pscustomobject]@{
                        Status = "Valid"
                        SignatureType = "Authenticode"
                        SignerCertificate = $otherCertificate
                    }
                }
            )
            foreach ($case in $identityCases) {
                $rejected = $false
                try {
                    Assert-AuthenticodeSigner $case.Signature $certificate
                } catch {
                    if ($_.Exception.Message -cne $case.Error) { throw }
                    $rejected = $true
                }
                if (-not $rejected) { throw "$($case.Name) self-test failed" }
            }

            $validSignature = [pscustomobject]@{
                Status = "Valid"
                SignatureType = "Authenticode"
                SignerCertificate = $certificate
            }
            Assert-AuthenticodeSigner $validSignature $certificate

            $routingTarget = Join-Path ([IO.Path]::GetTempPath()) "signing-routing-$PID.exe"
            [IO.File]::WriteAllBytes($routingTarget, [byte[]]@(0))
            $runnerCalls = [Collections.Generic.List[object]]::new()
            $routingCertificate = [Security.Cryptography.X509Certificates.X509Certificate2]::new(
                $certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert)
            )
            $routingState = [pscustomobject]@{
                Pfx = "routing-test.pfx"
                Password = "routing-password"
                TimestampUrl = "http://tsa.invalid.test"
                Certificate = $routingCertificate
            }
            $fakeRunner = {
                param($Exe, $RunnerArguments, $Timeout, $Phase, $RunnerTarget)
                $runnerCalls.Add([pscustomobject]@{
                    Executable = $Exe; Arguments = @($RunnerArguments); Timeout = $Timeout
                    Phase = $Phase; Target = $RunnerTarget
                })
                [pscustomobject]@{ ExitCode = 0 }
            }.GetNewClosure()
            $fakeSignature = [pscustomobject]@{
                Status = "Valid"
                SignatureType = "Authenticode"
                SignerCertificate = $routingCertificate
                TimeStamperCertificate = $certificate
            }
            try {
                $env:OS = "Windows_NT"
                $fakeStateReader = { $routingState }.GetNewClosure()
                $fakeSignatureReader = { param($Path) $fakeSignature }.GetNewClosure()
                Invoke-WindowsSign $routingTarget $fakeRunner $fakeStateReader `
                    { "fake-signtool.exe" } $fakeSignatureReader
                if ($runnerCalls.Count -ne 1 -or
                    $runnerCalls[0].Executable -cne "fake-signtool.exe" -or
                    $runnerCalls[0].Timeout -ne 120000 -or
                    $runnerCalls[0].Phase -cne "SignTool signing" -or
                    $runnerCalls[0].Target -cne $routingTarget -or
                    $runnerCalls[0].Arguments[-1] -cne $routingTarget) {
                    throw "Invoke-WindowsSign bounded-runner routing self-test failed"
                }

                $untimestampedCms = New-SelfTestCms $certificate "2.16.840.1.101.3.4.2.1" `
                    "2.16.840.1.101.3.4.2.1"
                $untimestampedState = [pscustomobject]@{
                    Pfx = "untimestamped-test.pfx"
                    Password = "untimestamped-password"
                    TimestampUrl = "http://tsa.invalid.test"
                    Certificate = [Security.Cryptography.X509Certificates.X509Certificate2]::new(
                        $certificate.Export(
                            [Security.Cryptography.X509Certificates.X509ContentType]::Cert))
                }
                $untimestampedStateReader = { $untimestampedState }.GetNewClosure()
                $untimestampedCmsReader = { $untimestampedCms }.GetNewClosure()
                $untimestampedSignatureReader = {
                    param($Path, $Tool, $Runner)
                    Get-EmbeddedAuthenticodeSignature $Path $Tool $Runner $untimestampedCmsReader
                }.GetNewClosure()
                $missingTimestampRejected = $false
                try {
                    Invoke-WindowsSign $routingTarget $fakeRunner $untimestampedStateReader `
                        { "fake-signtool.exe" } $untimestampedSignatureReader
                } catch {
                    if ($_.Exception.Message -cne
                        "Authenticode signature is missing its RFC 3161 timestamp") { throw }
                    $missingTimestampRejected = $true
                }
                if (-not $missingTimestampRejected) {
                    throw "Invoke-WindowsSign missing timestamp self-test failed"
                }
            } finally {
                Remove-Item -LiteralPath $routingTarget -Force -ErrorAction SilentlyContinue
            }
        } finally {
            $otherCertificate.Dispose()
        }
        $env:WINDOWS_SIGNING_CERTIFICATE_BASE64 = [Convert]::ToBase64String(
            $certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $password)
        )
        $env:WINDOWS_SIGNING_CERTIFICATE_PASSWORD = $password
        $env:WINDOWS_TIMESTAMP_URL = "https://timestamp.digicert.com"
        $env:GITHUB_ENV = $testEnvironment
        $script:OutputPfxPath = $testPfx
        $script:PersistEnvironment = $true
        Prepare-WindowsSigning
        $state = Get-PreparedSigningState
        $state.Certificate.Dispose()
        if ($state.TimestampUrl -ne "http://timestamp.digicert.com") {
            throw "prepared timestamp self-test failed"
        }
        $persisted = @(Get-Content -LiteralPath $testEnvironment)
        if ($persisted -notcontains "WINDOWS_CERTIFICATE_PFX_PATH=$testPfx" -or
            $persisted -notcontains "WINDOWS_SIGNING_TIMESTAMP_URL=http://timestamp.digicert.com") {
            throw "GitHub environment persistence self-test failed"
        }
    } finally {
        Remove-Item -LiteralPath $testPfx -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $testEnvironment -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $catalogTarget -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $sha1Target -Force -ErrorAction SilentlyContinue
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
