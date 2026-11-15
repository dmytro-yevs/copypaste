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

function Test-SelfSignedCertificate(
    [Security.Cryptography.X509Certificates.X509Certificate2]$Certificate
) {
    $subject = [Convert]::ToHexString($Certificate.SubjectName.RawData)
    $issuer = [Convert]::ToHexString($Certificate.IssuerName.RawData)
    return $subject -ceq $issuer
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
    $timestampCms.CheckSignature($true)
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

function Convert-EmbeddedSignatureCms([Security.Cryptography.Pkcs.SignedCms]$Cms) {
    if ($Cms.SignerInfos.Count -lt 1) {
        throw "Signed file did not expose an embedded Authenticode signature"
    }
    $Cms.CheckSignature($true)
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
    [scriptblock]$CmsReader = { param($Path) Read-EmbeddedSignatureCms $Path },
    [bool]$RequireTrustedChain = $true
) {
    if ($RequireTrustedChain) {
        $verification = & $ProcessRunner $SignTool @("verify", "/pa", "/all", "/tw", $Target) `
            120000 "SignTool embedded verification" $Target
        if ($verification.ExitCode -ne 0) {
            throw (Format-ProcessDiagnostic "SignTool embedded verification" $SignTool $Target `
                "exit $($verification.ExitCode)" $verification)
        }
    }

    return Convert-EmbeddedSignatureCms (& $CmsReader $Target)
}

function Assert-WindowsSignaturePolicy(
    [string]$Target,
    [object]$State,
    [string]$SignTool,
    [scriptblock]$ProcessRunner,
    [scriptblock]$SignatureReader
) {
    # /pa proves Windows policy trust only for CA-backed release certificates.
    $requireTrustedChain = -not (Test-SelfSignedCertificate $State.Certificate)
    $signature = & $SignatureReader $Target $SignTool $ProcessRunner $requireTrustedChain
    Assert-AuthenticodeSigner $signature $State.Certificate
    if ($null -eq $signature.TimeStamperCertificate) {
        throw "Authenticode signature is missing its RFC 3161 timestamp"
    }
}
