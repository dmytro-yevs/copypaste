$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
. (Join-Path $PSScriptRoot "windows-readiness-lib.ps1")
. (Join-Path $PSScriptRoot "windows-uia-snapshot-lib.ps1")
. (Join-Path $PSScriptRoot "windows-native-window-evidence.ps1")

function Get-AppAutomationRoot([Diagnostics.Process]$App) {
    $App.Refresh()
    if ($App.HasExited) { throw "installed app exited with code $($App.ExitCode)" }
    if ($App.MainWindowHandle -eq 0) { return $null }
    return [Windows.Automation.AutomationElement]::FromHandle($App.MainWindowHandle)
}

# UIAutomationTypes registers only the 39 control types it shipped with,
# 50000-50038. A provider reporting a later documented id (50039
# UIA_SemanticZoomControlTypeId, 50040 UIA_AppBarControlTypeId) sends
# Schema.ConvertToControlType through ControlType.LookupById, which returns
# $null for an id it never registered; StrictMode then raises "The property
# 'ProgrammaticName' cannot be found on this object". That is the client's
# naming table, not a node without a control type: a provider supplying no
# ControlType reads back as a real ControlType object, never as $null.
# Measured against a provider reporting each id in turn.
function Get-UiaControlTypeName([Windows.Automation.ControlType]$ControlType) {
    if ($null -eq $ControlType) { return $null }
    return $ControlType.ProgrammaticName
}

function Read-UiaNode([Windows.Automation.AutomationElement]$Element) {
    $bounds = $Element.Current.BoundingRectangle
    $coordinates = @($bounds.X, $bounds.Y, $bounds.Width, $bounds.Height)
    $serializedBounds = if (@($coordinates | Where-Object { [double]::IsNaN($_) -or [double]::IsInfinity($_) }).Count -eq 0) {
        [ordered]@{ x = $bounds.X; y = $bounds.Y; width = $bounds.Width; height = $bounds.Height }
    } else {
        $null
    }
    return [ordered]@{
        name = $Element.Current.Name
        control_type = Get-UiaControlTypeName $Element.Current.ControlType
        localized_control_type = $Element.Current.LocalizedControlType
        automation_id = $Element.Current.AutomationId
        enabled = $Element.Current.IsEnabled
        offscreen = $Element.Current.IsOffscreen
        bounds = $serializedBounds
    }
}

function Get-UiaSnapshot([Windows.Automation.AutomationElement]$Root) {
    $elements = @($Root) + @($Root.FindAll(
        [Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.Condition]::TrueCondition
    ))
    return Read-UiaSnapshot $elements { param($element) Read-UiaNode $element }
}

function Get-UiaSnapshotNames([Collections.IDictionary]$Snapshot) {
    return @(
        @($Snapshot["nodes"]) | ForEach-Object {
            if ($_ -is [Collections.IDictionary] -and $_.Contains("name") -and $_["name"]) {
                $_["name"]
            }
        } | Select-Object -First 40
    )
}

# Diagnostics, so a partial read is described rather than rejected: this runs
# while some other wait has already failed and is the only account of what the
# app was showing.
function Get-UiaSummary([Diagnostics.Process]$App) {
    $root = Get-AppAutomationRoot $App
    if ($null -eq $root) { return "native window handle is not ready" }
    $snapshot = Get-UiaSnapshot $root
    $names = @(Get-UiaSnapshotNames $snapshot)
    return "UIA names: $($names -join ' | ') [$(Get-UiaSnapshotReport $snapshot)]"
}

function Get-UiaNamedElement([Diagnostics.Process]$App, [string]$Name, [bool]$Actionable = $false) {
    $App.Refresh()
    if ($App.HasExited) { return $null }
    $root = Get-AppAutomationRoot $App
    if ($null -eq $root) { return $null }
    $condition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::NameProperty,
        $Name
    )
    try {
        $candidates = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
    } catch {
        return $null
    }
    foreach ($match in $candidates) {
        try {
            $bounds = $match.Current.BoundingRectangle
            $canAct = -not $Actionable -or $match.Current.IsKeyboardFocusable -or @(
                $match.GetSupportedPatterns() | Where-Object {
                    $_ -eq [Windows.Automation.InvokePattern]::Pattern -or
                    $_ -eq [Windows.Automation.SelectionItemPattern]::Pattern
                }
            ).Count -gt 0
            if ($canAct -and $match.Current.IsEnabled -and -not $match.Current.IsOffscreen -and $bounds.Width -gt 0 -and $bounds.Height -gt 0) {
                return $match
            }
        } catch {
            continue
        }
    }
    return $null
}

function Wait-UiaName([Diagnostics.Process]$App, [string]$Name, [bool]$Actionable = $false) {
    return Wait-Readiness "UI state '$Name'" {
        $App.Refresh()
        if ($App.HasExited) { return New-ProbeInvariant "the app exited with code $($App.ExitCode)" }
        $match = Get-UiaNamedElement $App $Name $Actionable
        if ($null -ne $match) { return New-ProbeReady $match }
        if ($null -eq (Get-AppAutomationRoot $App)) { return New-ProbeNotReady "the app has published no native window handle" }
        return New-ProbeNotReady "no enabled, on-screen element is named '$Name'"
    } { Get-UiaSummary $App } 15000
}

function Invoke-UiaElement([Windows.Automation.AutomationElement]$Element, [string]$Name) {
    $pattern = $null
    if ($Element.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        ([Windows.Automation.InvokePattern]$pattern).Invoke()
    } elseif ($Element.TryGetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) {
        ([Windows.Automation.SelectionItemPattern]$pattern).Select()
    } elseif ($Element.Current.IsKeyboardFocusable) {
        $Element.SetFocus()
        [Windows.Forms.SendKeys]::SendWait("{ENTER}")
    } else {
        $supported = @($Element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName }) -join ", "
        throw "UI control '$Name' has no actionable accessibility pattern; supported: $supported"
    }
}

function Complete-WindowsFirstRun([Diagnostics.Process]$App) {
    Wait-Readiness "welcome dismissed or product shell" {
        $App.Refresh()
        if ($App.HasExited) { return New-ProbeInvariant "the app exited with code $($App.ExitCode)" }
        if ($null -ne (Get-UiaNamedElement $App "Preferences" $true)) {
            return New-ProbeReady $true
        }
        $explore = Get-UiaNamedElement $App "Explore first" $true
        if ($null -ne $explore) {
            Invoke-UiaElement $explore "Explore first"
            return New-ProbeNotReady "welcome dismissed"
        }
        return New-ProbeNotReady "neither Explore first nor Preferences is on screen"
    } { Get-UiaSummary $App } 20000 | Out-Null
}

function Invoke-UiaNamedControl([Diagnostics.Process]$App, [string]$Name, [string]$ExpectedName) {
    $element = Wait-UiaName $App $Name $true
    Invoke-UiaElement $element $Name
    Wait-UiaName $App $ExpectedName | Out-Null
}

function Get-WindowsPairingEntryState(
    [bool]$CodeVisible,
    [bool]$AddressVisible,
    [bool]$JoinVisible,
    [bool]$JoinInvoked
) {
    if ($CodeVisible -and $AddressVisible) { return "ready" }
    if ($JoinVisible -and -not $JoinInvoked) { return "invoke" }
    return "wait"
}

function Open-WindowsPairingEntry([Diagnostics.Process]$App) {
    $launcher = Wait-UiaName $App "Connect a device" $true
    Invoke-UiaElement $launcher "Connect a device"
    $transition = @{ join_invoked = $false }
    Wait-Readiness "native pairing entry" {
        $App.Refresh()
        if ($App.HasExited) { return New-ProbeInvariant "the app exited with code $($App.ExitCode)" }
        $code = Get-UiaNamedElement $App "Pairing code"
        $address = Get-UiaNamedElement $App "Pairing address"
        $join = Get-UiaNamedElement $App "Enter pairing code" $true
        switch (Get-WindowsPairingEntryState ($null -ne $code) ($null -ne $address) ($null -ne $join) $transition.join_invoked) {
            "ready" { return New-ProbeReady $true }
            "invoke" {
                $transition.join_invoked = $true
                Invoke-UiaElement $join "Enter pairing code"
                return New-ProbeNotReady "the native pairing entry was requested"
            }
        }
        return New-ProbeNotReady "neither the pairing launcher action nor the native pairing fields are visible"
    } { Get-UiaSummary $App } 15000 | Out-Null
}

function Set-UiaScreenshots(
    [Diagnostics.Process]$App,
    [bool]$Allow,
    [string]$CaptureTracePath = ""
) {
    $element = Wait-UiaName $App "Allow screenshots" $true
    $pattern = $null
    if (-not $element.TryGetCurrentPattern([Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
        $supported = @($element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName }) -join ", "
        throw "Allow screenshots has no Toggle pattern; supported: $supported"
    }
    $toggle = [Windows.Automation.TogglePattern]$pattern
    $expected = if ($Allow) { [Windows.Automation.ToggleState]::On } else { [Windows.Automation.ToggleState]::Off }
    if ($toggle.Current.ToggleState -ne $expected) { $toggle.Toggle() }
    $state = Wait-Readiness "Allow screenshots=$Allow native state" {
        $App.Refresh()
        if ($App.HasExited) { return New-ProbeInvariant "the app exited with code $($App.ExitCode)" }
        try {
            $observed = $toggle.Current.ToggleState
        } catch {
            return New-ProbeTransient "the toggle state could not be read: $($_.Exception.Message)"
        }
        if ($observed -ne $expected) { return New-ProbeNotReady "the toggle reads $observed" }
        $state = Get-WindowCaptureState $App.MainWindowHandle
        if (($Allow -and $state.capture_allowed) -or (-not $Allow -and $state.display_affinity -gt 0)) {
            return New-ProbeReady $state
        }
        return New-ProbeNotReady "display affinity is $($state.display_affinity)"
    } {
        $state = Get-WindowCaptureState $App.MainWindowHandle
        @(Get-UiaSummary $App; "display affinity=$($state.display_affinity)")
    } 15000
    Write-WindowCaptureObservation $App $CaptureTracePath "screenshots/after-toggle" $state
}

function New-EvidenceFileRecord([string]$Root, [string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    return [ordered]@{
        path = $RelativePath.Replace('\', '/')
        sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        bytes = (Get-Item -LiteralPath $path).Length
    }
}

function Save-WindowsFeatureState(
    [Diagnostics.Process]$App,
    [string]$EvidenceRoot,
    [string]$Feature,
    [string]$State,
    [string]$ExpectedName,
    [string]$ArtifactDirectory = "",
    [string]$CaptureTracePath = ""
) {
    Wait-UiaName $App $ExpectedName | Out-Null
    $root = Get-AppAutomationRoot $App
    $snapshot = Get-UiaSnapshot $root
    Assert-UiaSnapshotComplete $snapshot "$Feature/$State evidence"
    $nodes = $snapshot.nodes
    $markers = @($nodes | Where-Object {
        if ($_ -isnot [Collections.IDictionary]) { return $false }
        $bounds = $_["bounds"]
        return $_["name"] -eq $ExpectedName -and $_["enabled"] -and -not $_["offscreen"] -and
            $bounds -is [Collections.IDictionary] -and $bounds["width"] -gt 0 -and $bounds["height"] -gt 0
    })
    Assert-True ($markers.Count -gt 0) "$Feature evidence lacks a visible, enabled '$ExpectedName' marker"
    $relativeDirectory = if ($ArtifactDirectory) { Join-Path $Feature $ArtifactDirectory } else { $Feature }
    $directory = Join-Path $EvidenceRoot $relativeDirectory
    [IO.Directory]::CreateDirectory($directory) | Out-Null
    $screenshot = Join-Path $relativeDirectory "screenshot.png"
    $accessibility = Join-Path $relativeDirectory "accessibility.json"
    $window = Save-WindowImage $App (Join-Path $EvidenceRoot $screenshot) $CaptureTracePath "$Feature/$State"
    [ordered]@{
        schema_version = 2
        feature = $Feature
        state = $State
        expected_name = $ExpectedName
        window = $window
        node_read = [ordered]@{
            complete = $true
            read = @($nodes).Count
            retried = @($snapshot.retried)
        }
        nodes = $nodes
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceRoot $accessibility) -Encoding utf8
    return [ordered]@{
        feature = $Feature
        state = $State
        expected_name = $ExpectedName
        screenshot = New-EvidenceFileRecord $EvidenceRoot $screenshot
        accessibility = New-EvidenceFileRecord $EvidenceRoot $accessibility
    }
}

function Write-WindowsFeatureManifest([string]$EvidenceRoot, [object[]]$States) {
    [ordered]@{ schema_version = 1; states = $States } |
        ConvertTo-Json -Depth 8 |
        Set-Content -LiteralPath (Join-Path $EvidenceRoot "feature-states.json") -Encoding utf8
}

function Test-WindowsUiEvidenceHelpers {
    Test-UiaSnapshotHelpers
    $names = @(Get-UiaSnapshotNames ([ordered]@{
        nodes = @([ordered]@{ name = "Explore first" }, [ordered]@{ control_type = "ControlType.Button" })
    }))
    Assert-True ($names.Count -eq 1 -and $names[0] -eq "Explore first") `
        "UIA diagnostics did not tolerate a node without a name"
    Assert-True ((Get-UiaControlTypeName ([Windows.Automation.ControlType]::Button)) -eq "ControlType.Button") `
        "a registered control type was not read from the element"
    Assert-True ($null -eq (Get-UiaControlTypeName $null)) `
        "a control type the client cannot name was given a name anyway"
    Assert-True ((Get-WindowsPairingEntryState $true $true $false $false) -eq "ready") `
        "both native pairing fields did not identify the entry state"
    Assert-True ((Get-WindowsPairingEntryState $false $false $true $false) -eq "invoke") `
        "the launcher action did not advance the pairing entry state"
    Assert-True ((Get-WindowsPairingEntryState $false $false $true $true) -eq "wait") `
        "the launcher action could be invoked more than once"
    Assert-True ((Get-WindowsPairingEntryState $true $false $false $false) -eq "wait") `
        "a partial native pairing form was accepted as ready"
    $occluded = [ordered]@{ foreground = $false; visible = $true; minimized = $false; capture_allowed = $true }
    Assert-True (-not (Test-WindowCaptureReady $occluded)) "an occluded window was accepted for capture"
    $protected = [ordered]@{ foreground = $true; visible = $true; minimized = $false; capture_allowed = $false }
    Assert-True (-not (Test-WindowCaptureReady $protected)) "a capture-protected window was accepted for capture"
    $settled = [ordered]@{ foreground = $true; visible = $true; minimized = $false }
    $settledPlan = Get-WindowActivationPlan $settled
    Assert-True (-not $settledPlan.restore -and -not $settledPlan.activate) `
        "a settled window was mutated before capture"
    $minimized = [ordered]@{ foreground = $false; visible = $true; minimized = $true }
    $minimizedPlan = Get-WindowActivationPlan $minimized
    Assert-True ($minimizedPlan.restore -and $minimizedPlan.activate) `
        "a minimized background window was not restored and activated"
    $observation = New-WindowCaptureObservation "fixture/pre-capture" ([ordered]@{
        handle = 41; foreground = $true; visible = $true; minimized = $false
        capture_allowed = $true; display_affinity = 0
    })
    Assert-True ($observation.phase -eq "fixture/pre-capture" -and $observation.handle -eq 41 -and
        $observation.capture_allowed -and $observation.display_affinity -eq 0) `
        "capture-affinity diagnostics lost the measured fixture state"
}
