# SignTool /tr (RFC 3161) rejects https:// for common public TSAs with
# "Invalid Timestamp URL" (alpha.27). Release secrets were forced to https://
# by an older HTTPS-only check; map known hosts to their documented http endpoints.
function Resolve-SignToolTimestampUrl {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$Raw
    )
    if ([string]::IsNullOrWhiteSpace($Raw)) {
        throw "WINDOWS_TIMESTAMP_URL is required"
    }
    $trimmed = $Raw.Trim()
    try {
        $uri = [Uri]::new($trimmed)
    } catch {
        throw "WINDOWS_TIMESTAMP_URL is not a valid URI"
    }
    if ($uri.Scheme -notin @("http", "https") -or [string]::IsNullOrWhiteSpace($uri.Host)) {
        throw "WINDOWS_TIMESTAMP_URL must be an http(s) URL with a host"
    }
    $hostKey = $uri.Host.ToLowerInvariant()
    $httpTsa = @{
        "timestamp.digicert.com" = "http://timestamp.digicert.com"
        "timestamp.sectigo.com" = "http://timestamp.sectigo.com"
        "timestamp.globalsign.com" = "http://timestamp.globalsign.com/tsa/r6advanced1"
        "timestamp.comodoca.com" = "http://timestamp.comodoca.com"
        "timestamp.apple.com" = "http://timestamp.apple.com/ts01"
    }
    if ($httpTsa.ContainsKey($hostKey)) {
        return $httpTsa[$hostKey]
    }
    if ($uri.Scheme -eq "https") {
        return "http://$($uri.Authority)$($uri.PathAndQuery)"
    }
    return $trimmed
}

if ($MyInvocation.InvocationName -ne "." -and $args -contains "-SelfTest") {
    $ErrorActionPreference = "Stop"
    Set-StrictMode -Version Latest
    $cases = @(
        @{ In = "https://timestamp.digicert.com"; Out = "http://timestamp.digicert.com" },
        @{ In = " http://timestamp.digicert.com "; Out = "http://timestamp.digicert.com" },
        @{ In = "https://timestamp.sectigo.com/rfc3161"; Out = "http://timestamp.sectigo.com" },
        @{ In = "https://tsa.example.test/path"; Out = "http://tsa.example.test/path" }
    )
    foreach ($case in $cases) {
        $got = Resolve-SignToolTimestampUrl $case.In
        if ($got -ne $case.Out) {
            throw "Resolve-SignToolTimestampUrl('$($case.In)') => '$got', expected '$($case.Out)'"
        }
    }
    $failed = $false
    try {
        Resolve-SignToolTimestampUrl "" | Out-Null
        $failed = $true
    } catch {
        if ("$_" -notmatch "required") { throw }
    }
    if ($failed) { throw "empty timestamp URL must fail" }
    Write-Host "PASS: SignTool timestamp URL resolution"
}
