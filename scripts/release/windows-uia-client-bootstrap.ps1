$script:WindowsUiaClientProviderReady = $false
$script:WindowsUiaClientProviderBootstrap = $null
$script:WindowsUiaClientProviderCanaryDiagnostic = $null

function Get-WindowsUiaProviderAssemblyName([Reflection.AssemblyName]$ClientAssembly) {
    $provider = [Reflection.AssemblyName]::new()
    $provider.Name = "UIAutomationClientsideProviders"
    $provider.Version = $ClientAssembly.Version
    $provider.CultureInfo = $ClientAssembly.CultureInfo
    $provider.SetPublicKeyToken($ClientAssembly.GetPublicKeyToken())
    return $provider
}

function Format-WindowsUiaAssemblyIdentity([Reflection.AssemblyName]$AssemblyName) {
    $token = @($AssemblyName.GetPublicKeyToken() | ForEach-Object { $_.ToString("x2") }) -join ""
    return "$($AssemblyName.Name), Version=$($AssemblyName.Version), PublicKeyToken=$token"
}

function Get-LoadedWindowsUiaProviderIdentities {
    return @(
        [AppDomain]::CurrentDomain.GetAssemblies() |
            Where-Object { $_.GetName().Name -eq "UIAutomationClientsideProviders" } |
            ForEach-Object { Format-WindowsUiaAssemblyIdentity $_.GetName() } |
            Sort-Object -Unique
    )
}

function New-WindowsUiaCanaryExpectation(
    [string]$Role,
    [string]$ClassName,
    [string]$ControlType,
    [bool]$IsPassword,
    [string]$ConfiguredStyleFlags
) {
    return [ordered]@{
        role = $Role
        class_name = $ClassName
        control_type = $ControlType
        is_password = $IsPassword
        configured_style_flags = $ConfiguredStyleFlags
    }
}

function Get-WindowsUiaCanaryExpectations {
    return @(
        New-WindowsUiaCanaryExpectation "password-edit" "Edit" "ControlType.Edit" $true `
            "WS_CHILD|WS_VISIBLE|WS_BORDER|ES_PASSWORD"
        New-WindowsUiaCanaryExpectation "button" "Button" "ControlType.Button" $false `
            "WS_CHILD|WS_VISIBLE|WS_BORDER"
        New-WindowsUiaCanaryExpectation "static" "Static" "ControlType.Text" $false `
            "WS_CHILD|WS_VISIBLE|WS_BORDER"
    )
}

function Test-WindowsUiaCanaryMappings([object[]]$Facts) {
    $expected = @(Get-WindowsUiaCanaryExpectations)
    if (@($Facts).Count -ne $expected.Count) { return $false }
    foreach ($requirement in $expected) {
        $matches = @($Facts | Where-Object {
            $_ -is [Collections.IDictionary] -and
            $_["role"] -eq $requirement["role"] -and
            $_["class_name"] -eq $requirement["class_name"] -and
            $_["control_type"] -eq $requirement["control_type"] -and
            $_["is_password"] -eq $requirement["is_password"] -and
            $_["configured_style_flags"] -eq $requirement["configured_style_flags"]
        })
        if ($matches.Count -ne 1) { return $false }
    }
    return $true
}

function New-WindowsUiaCanaryDiagnostic([string]$Phase, [string]$Outcome, [object[]]$Controls) {
    return [ordered]@{
        schema_version = 1
        phase = $Phase
        outcome = $Outcome
        controls = @($Controls)
    }
}

function Write-WindowsUiaCanaryDiagnostic([Collections.IDictionary]$Diagnostic) {
    Write-Information ("Windows UIA canary diagnostics: " + ($Diagnostic | ConvertTo-Json -Compress -Depth 6)) `
        -InformationAction Continue
}

function Resolve-WindowsUiaFixtureReference([string[]]$Names) {
    foreach ($name in $Names) {
        $reference = Join-Path $PSHOME (Join-Path "ref" "$name.dll")
        if (Test-Path -LiteralPath $reference -PathType Leaf) { return $reference }
        $loaded = [AppDomain]::CurrentDomain.GetAssemblies() | Where-Object {
            $_.GetName().Name -eq $name -and $_.Location
        } | Select-Object -First 1
        if ($null -ne $loaded) { return $loaded.Location }
        try {
            $assembly = [Reflection.Assembly]::Load($name)
            if ($assembly.Location) { return $assembly.Location }
        } catch { }
    }
    throw "Windows UIA canary compiler reference is unavailable"
}

function Get-WindowsUiaFixtureReferences {
    $referenceSets = @(
        @("System.Runtime", "mscorlib"),
        @("System.Runtime.InteropServices", "mscorlib"),
        @("System.Threading", "mscorlib"),
        @("System.Threading.Thread", "mscorlib"),
        @("System.Windows.Forms"),
        @("System.Windows.Forms.Primitives", "System.Windows.Forms"),
        @("System.Drawing", "System.Drawing.Common"),
        @("System.Drawing.Primitives", "System.Drawing"),
        @("System.ComponentModel.Primitives", "System"),
        @("System.Collections", "mscorlib"),
        @("System.ObjectModel", "System")
    )
    return @($referenceSets | ForEach-Object { Resolve-WindowsUiaFixtureReference $_ } | Select-Object -Unique)
}

function Add-WindowsUiaCanaryFixtureType {
    if ($null -ne ("CopyPaste.UiaCanary.Session" -as [type])) { return }
    Add-Type -Path (Join-Path $PSScriptRoot "windows-uia-canary-fixture.cs") `
        -ReferencedAssemblies @(Get-WindowsUiaFixtureReferences)
}

function Read-WindowsUiaCanaryControl(
    [IntPtr]$Handle,
    [Collections.IDictionary]$Expectation
) {
    $element = [Windows.Automation.AutomationElement]::FromHandle($Handle)
    return [ordered]@{
        role = $Expectation["role"]
        class_name = $element.Current.ClassName
        control_type = $element.Current.ControlType.ProgrammaticName
        is_password = $element.Current.IsPassword
        configured_style_flags = $Expectation["configured_style_flags"]
    }
}

function Invoke-WindowsUiaCanaryProbe(
    [scriptblock]$Start,
    [scriptblock]$Read,
    [scriptblock]$Close
) {
    $session = $null
    $facts = @()
    $phase = "fixture-start"
    $primaryFailed = $false
    $cleanupFailed = $false
    try {
        $session = & $Start
        $phase = "control-read"
        $facts = @(& $Read $session)
        $phase = "mapping-check"
        if (-not (Test-WindowsUiaCanaryMappings $facts)) { throw "UIA canary provider mapping mismatch" }
        $phase = "complete"
    } catch {
        $primaryFailed = $true
    } finally {
        if ($null -ne $session) {
            try { & $Close $session } catch { $cleanupFailed = $true }
        }
    }
    $outcome = if ($primaryFailed -or $cleanupFailed) { "failed" } else { "ready" }
    $script:WindowsUiaClientProviderCanaryDiagnostic = New-WindowsUiaCanaryDiagnostic $phase $outcome $facts
    Write-WindowsUiaCanaryDiagnostic $script:WindowsUiaClientProviderCanaryDiagnostic
    if ($primaryFailed) {
        if ($cleanupFailed) { Write-Warning "Windows UIA canary cleanup failed; preserving the primary observer failure." }
        throw "Windows UI Automation client provider canary failed. The native control observer is unavailable."
    }
    if ($cleanupFailed) {
        throw "Windows UI Automation client provider canary cleanup failed. The native control observer is unavailable."
    }
    return $facts
}

function Invoke-WindowsUiaClientProviderCanary {
    Add-WindowsUiaCanaryFixtureType
    $probe = @{
        Start = { [CopyPaste.UiaCanary.Session]::Start() }
        Read = {
            param($session)
            $expected = @(Get-WindowsUiaCanaryExpectations)
            @(
                Read-WindowsUiaCanaryControl $session.PasswordEditHandle $expected[0]
                Read-WindowsUiaCanaryControl $session.ButtonHandle $expected[1]
                Read-WindowsUiaCanaryControl $session.StaticHandle $expected[2]
            )
        }
        Close = { param($session) $session.Dispose() }
    }
    return Invoke-WindowsUiaCanaryProbe @probe
}

function New-WindowsUiaProviderBootstrapDiagnostic(
    [string]$Phase,
    [string]$Outcome,
    [Reflection.AssemblyName]$Client,
    [Reflection.AssemblyName]$Provider,
    [string[]]$Before,
    [string[]]$After,
    [Collections.IDictionary]$Canary
) {
    return [ordered]@{
        schema_version = 1
        phase = $Phase
        outcome = $Outcome
        client_assembly = Format-WindowsUiaAssemblyIdentity $Client
        requested_provider_assembly = Format-WindowsUiaAssemblyIdentity $Provider
        provider_assemblies_before = @($Before)
        provider_assemblies_after = @($After)
        canary = $Canary
    }
}

function Write-WindowsUiaProviderBootstrapDiagnostic([Collections.IDictionary]$Diagnostic) {
    Write-Information ("Windows UIA provider diagnostics: " + ($Diagnostic | ConvertTo-Json -Compress -Depth 8)) `
        -InformationAction Continue
}

function Initialize-WindowsUiaClientProviders {
    if ($script:WindowsUiaClientProviderReady) { return $script:WindowsUiaClientProviderBootstrap }
    $client = [Windows.Automation.ClientSettings].Assembly.GetName()
    $provider = Get-WindowsUiaProviderAssemblyName $client
    $before = @(Get-LoadedWindowsUiaProviderIdentities)
    $after = @()
    $phase = "provider-registration"
    try {
        [Windows.Automation.ClientSettings]::RegisterClientSideProviderAssembly($provider)
        $after = @(Get-LoadedWindowsUiaProviderIdentities)
        if ($after.Count -eq 0) { throw "client-side provider assembly is not loaded" }
        $phase = "native-control-canary"
        $canary = @(Invoke-WindowsUiaClientProviderCanary)
        if (-not (Test-WindowsUiaCanaryMappings $canary)) { throw "client-side provider canary mismatch" }
        $phase = "complete"
    } catch {
        $script:WindowsUiaClientProviderBootstrap = New-WindowsUiaProviderBootstrapDiagnostic `
            $phase "failed" $client $provider $before $after $script:WindowsUiaClientProviderCanaryDiagnostic
        Write-WindowsUiaProviderBootstrapDiagnostic $script:WindowsUiaClientProviderBootstrap
        throw "Windows UI Automation environment bootstrap failed: explicit client-side provider registration and the native-control canary must succeed before product evidence runs."
    }
    $script:WindowsUiaClientProviderReady = $true
    $script:WindowsUiaClientProviderBootstrap = New-WindowsUiaProviderBootstrapDiagnostic `
        $phase "ready" $client $provider $before $after $script:WindowsUiaClientProviderCanaryDiagnostic
    Write-WindowsUiaProviderBootstrapDiagnostic $script:WindowsUiaClientProviderBootstrap
    return $script:WindowsUiaClientProviderBootstrap
}
