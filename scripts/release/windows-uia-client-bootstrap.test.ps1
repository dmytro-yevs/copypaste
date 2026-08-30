function Test-WindowsUiaClientBootstrapHelpers {
    $client = [Reflection.AssemblyName]::new("UIAutomationClient, Version=4.0.0.0, Culture=neutral, PublicKeyToken=31bf3856ad364e35")
    $provider = Get-WindowsUiaProviderAssemblyName $client
    if ($provider.Name -ne "UIAutomationClientsideProviders" -or $provider.Version -ne $client.Version) {
        throw "UIA provider identity did not retain the compatible client identity"
    }
    $facts = @(Get-WindowsUiaCanaryExpectations)
    if (-not (Test-WindowsUiaCanaryMappings $facts)) { throw "the native UIA canary expectations were rejected" }
    if (Test-WindowsUiaCanaryMappings @($facts | Select-Object -Skip 1)) { throw "the native UIA canary accepted a missing control" }
    if (Test-WindowsUiaCanaryMappings @($facts + $facts[0])) { throw "the native UIA canary accepted a duplicate control" }
    $wrongType = @(Get-WindowsUiaCanaryExpectations)
    $wrongType[1]["control_type"] = "ControlType.Pane"
    if (Test-WindowsUiaCanaryMappings $wrongType) { throw "the native UIA canary accepted a wrong control type" }
    Invoke-WindowsUiaCanaryProbe { [pscustomobject]@{} } { param($session) @(Get-WindowsUiaCanaryExpectations) } `
        { param($session) } | Out-Null
    if ($script:WindowsUiaClientProviderCanaryDiagnostic.phase -ne "complete" -or
        $script:WindowsUiaClientProviderCanaryDiagnostic.outcome -ne "ready") {
        throw "the UIA canary did not retain a ready diagnostic"
    }
    $mappingFailure = try {
        Invoke-WindowsUiaCanaryProbe { [pscustomobject]@{} } { param($session) $wrongType } { param($session) } | Out-Null
    } catch { $_.Exception.Message }
    if ($mappingFailure -ne "Windows UI Automation client provider canary failed. The native control observer is unavailable." -or
        $script:WindowsUiaClientProviderCanaryDiagnostic.phase -ne "mapping-check" -or
        @($script:WindowsUiaClientProviderCanaryDiagnostic.controls).Count -ne 3) {
        throw "the UIA canary discarded observed mapping facts on failure"
    }
    $closed = [ordered]@{ called = $false }
    $cleanupFailure = try {
        Invoke-WindowsUiaCanaryProbe { [pscustomobject]@{} } { param($session) @(Get-WindowsUiaCanaryExpectations) } `
            { param($session) $closed["called"] = $true; throw "fixture cleanup failure" } | Out-Null
    } catch { $_.Exception.Message }
    if (-not $closed["called"] -or $cleanupFailure -ne "Windows UI Automation client provider canary cleanup failed. The native control observer is unavailable.") {
        throw "the UIA canary accepted a cleanup failure"
    }
    $warnings = @()
    $primaryAndCleanup = try {
        Invoke-WindowsUiaCanaryProbe { [pscustomobject]@{} } { param($session) throw "C:\\private\\read-failure" } `
            { param($session) throw "fixture cleanup failure" } -WarningVariable +warnings | Out-Null
    } catch { $_.Exception.Message }
    if ($primaryAndCleanup -ne "Windows UI Automation client provider canary failed. The native control observer is unavailable." -or
        $warnings -notmatch "preserving the primary observer failure") {
        throw "the UIA canary did not preserve the primary failure"
    }
    if ($script:WindowsUiaClientProviderCanaryDiagnostic.phase -ne "control-read" -or
        $script:WindowsUiaClientProviderCanaryDiagnostic.outcome -ne "failed") {
        throw "the UIA canary discarded its failure phase"
    }
    $source = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "windows-native-ui-evidence.ps1"))
    $bootstrap = $source.IndexOf("Initialize-WindowsUiaClientProviders")
    $productElement = $source.IndexOf("[Windows.Automation.AutomationElement]::FromHandle")
    if ($bootstrap -lt 0 -or $bootstrap -ge $productElement) {
        throw "UIA provider bootstrap did not precede the first product AutomationElement"
    }
    if (${function:Invoke-WindowsUiaCanaryProbe}.ToString() -match "ValuePattern|Exception.Message|Screenshot") {
        throw "UIA provider bootstrap exposed a protected value, raw exception, or screenshot path"
    }
    $referenceSource = ${function:Get-WindowsUiaFixtureReferences}.ToString()
    foreach ($name in @(
        "System.Runtime", "System.Runtime.InteropServices", "System.Threading", "System.Threading.Thread",
        "System.Windows.Forms", "System.Windows.Forms.Primitives", "System.Drawing", "System.Drawing.Primitives",
        "System.ComponentModel.Primitives", "System.Collections", "System.ObjectModel"
    )) {
        if ($referenceSource -notmatch [regex]::Escape($name)) { throw "the UIA fixture omitted an explicit compiler reference" }
    }
}
