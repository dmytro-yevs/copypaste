$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
. (Join-Path $PSScriptRoot "windows-readiness-lib.ps1")
. (Join-Path $PSScriptRoot "windows-uia-snapshot-lib.ps1")
. (Join-Path $PSScriptRoot "windows-native-window-evidence.ps1")
. (Join-Path $PSScriptRoot "windows-native-protected-evidence.ps1")
. (Join-Path $PSScriptRoot "windows-protected-failure-diagnostics.ps1")
. (Join-Path $PSScriptRoot "windows-protected-failure-diagnostics.test.ps1")
. (Join-Path $PSScriptRoot "windows-uia-client-bootstrap.ps1")
. (Join-Path $PSScriptRoot "windows-uia-client-bootstrap.test.ps1")

if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
    Initialize-WindowsUiaClientProviders | Out-Null
}

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
$UIA_NAMED_DIAGNOSTIC_CANDIDATE_LIMIT = 8
$UIA_NAMED_DIAGNOSTIC_MAXIMUM_LENGTH = 4096
$UIA_OFFSCREEN_SCROLL_FAILURE = "the offscreen UI Automation element could not be revealed"

function Get-UiaDiagnosticBounds($Bounds) {
    $coordinates = @($Bounds.X, $Bounds.Y, $Bounds.Width, $Bounds.Height)
    if (@($coordinates | Where-Object { [double]::IsNaN($_) -or [double]::IsInfinity($_) }).Count -gt 0) {
        return "non-finite"
    }
    return "($($Bounds.X),$($Bounds.Y),$($Bounds.Width),$($Bounds.Height))"
}
function Format-UiaNamedCandidateDiagnostics([string]$Name, [int]$MatchCount, [object[]]$Candidates) {
    $parts = @("UIA exact-name candidates for '$Name': $MatchCount match(es); showing $(@($Candidates).Count) of $MatchCount")
    $index = 0
    foreach ($candidate in @($Candidates)) {
        $index++
        if ($candidate["status"] -eq "stale-or-unreadable") {
            $parts += "candidate[$index]=stale-or-unreadable"
            continue
        }
        $patterns = @($candidate["patterns"] | Select-Object -First 5) -join ","
        $parts += ("candidate[{0}]: control_type={1}; enabled={2}; offscreen={3}; bounds={4}; keyboard_focusable={5}; relevant_patterns={6}; actionable={7}" -f @(
            $index, $candidate["control_type"], $candidate["enabled"], $candidate["offscreen"], $candidate["bounds"],
            $candidate["keyboard_focusable"], $patterns, $candidate["actionable"]
        ))
    }
    $text = $parts -join "; "
    if ($text.Length -gt $UIA_NAMED_DIAGNOSTIC_MAXIMUM_LENGTH) {
        return $text.Substring(0, $UIA_NAMED_DIAGNOSTIC_MAXIMUM_LENGTH - 12) + " [truncated]"
    }
    return $text
}
# Failure-only evidence excludes candidate Name, Value, AutomationId,
# exception text, and arbitrary provider data.
function Get-UiaNamedCandidateDiagnostics([Diagnostics.Process]$App, [string]$Name, [bool]$Actionable = $false) {
    $App.Refresh()
    if ($App.HasExited) { return "UIA exact-name candidates: the app exited" }
    $root = Get-AppAutomationRoot $App
    if ($null -eq $root) { return "UIA exact-name candidates: native window handle is not ready" }
    $condition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::NameProperty,
        $Name
    )
    try {
        $matches = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
    } catch {
        return "UIA exact-name candidates for '$Name': scan unavailable"
    }
    try { $count = $matches.Count } catch {
        return "UIA exact-name candidates for '$Name': scan unavailable"
    }
    $records = @()
    for ($index = 0; $index -lt [Math]::Min($count, $UIA_NAMED_DIAGNOSTIC_CANDIDATE_LIMIT); $index++) {
        try {
            $match = $matches[$index]
            $controlType = try { Get-UiaControlTypeName $match.Current.ControlType } catch { "unavailable" }
            $patterns = @()
            foreach ($pattern in @($match.GetSupportedPatterns())) {
                if ($pattern -eq [Windows.Automation.InvokePattern]::Pattern) { $patterns += "invoke" }
                if ($pattern -eq [Windows.Automation.SelectionItemPattern]::Pattern) { $patterns += "selection-item" }
                if ($pattern -eq [Windows.Automation.TogglePattern]::Pattern) { $patterns += "toggle" }
                if ($pattern -eq [Windows.Automation.ScrollItemPattern]::Pattern) { $patterns += "scroll-item" }
                if ($pattern -eq [Windows.Automation.ScrollPattern]::Pattern) { $patterns += "scroll" }
            }
            $keyboardFocusable = $match.Current.IsKeyboardFocusable
            $canAct = -not $Actionable -or $keyboardFocusable -or
                $patterns -contains "invoke" -or $patterns -contains "selection-item"
            $records += [ordered]@{
                control_type = $controlType
                enabled = $match.Current.IsEnabled
                offscreen = $match.Current.IsOffscreen
                bounds = Get-UiaDiagnosticBounds $match.Current.BoundingRectangle
                keyboard_focusable = $keyboardFocusable
                patterns = @($patterns)
                actionable = $canAct
            }
        } catch {
            $records += [ordered]@{ status = "stale-or-unreadable" }
        }
    }
    return Format-UiaNamedCandidateDiagnostics $Name $count $records
}
function Get-UiaNamedWaitDiagnostics($App, [string]$Name, [bool]$Actionable = $false, [scriptblock]$Summary = { param($Process) Get-UiaSummary $Process }, [scriptblock]$Candidates = { param($Process, $Target, $RequireAction) Get-UiaNamedCandidateDiagnostics $Process $Target $RequireAction }) {
    $summaryReport = try { & $Summary $App } catch { "UIA summary unavailable" }
    $candidateReport = try { & $Candidates $App $Name $Actionable } catch { "UIA exact-name candidates unavailable" }
    return @($summaryReport, $candidateReport)
}
function Test-UiaFinitePositiveBounds($Bounds) {
    if ($null -eq $Bounds) { return $false }
    $coordinates = @($Bounds.X, $Bounds.Y, $Bounds.Width, $Bounds.Height)
    if (@($coordinates | Where-Object { [double]::IsNaN($_) -or [double]::IsInfinity($_) }).Count -gt 0) {
        return $false
    }
    return $Bounds.Width -gt 0 -and $Bounds.Height -gt 0
}

function Test-UiaNamedCandidateReady([Collections.IDictionary]$Candidate) {
    return $null -ne $Candidate -and $Candidate["enabled"] -and -not $Candidate["offscreen"] -and
        $Candidate["actionable"] -and (Test-UiaFinitePositiveBounds $Candidate["bounds"])
}

function Test-UiaNamedCandidateScrollable([Collections.IDictionary]$Candidate) {
    return $null -ne $Candidate -and $Candidate["enabled"] -and $Candidate["offscreen"] -and
        $Candidate["actionable"] -and $null -ne $Candidate["scroll_item"] -and
        (Test-UiaFinitePositiveBounds $Candidate["bounds"])
}

function Test-UiaScrollItemPatternNeeded([bool]$ScrollOptIn, [bool]$Offscreen) {
    return $ScrollOptIn -and $Offscreen
}

function Get-UiaOptionalScrollItemPattern(
    [bool]$ScrollOptIn,
    [bool]$Offscreen,
    [scriptblock]$ReadPattern
) {
    if (-not (Test-UiaScrollItemPatternNeeded $ScrollOptIn $Offscreen)) { return $null }
    try {
        return & $ReadPattern
    } catch {
        return $null
    }
}

function Find-UiaReadyNamedCandidate([object[]]$Candidates) {
    foreach ($candidate in @($Candidates)) {
        if (Test-UiaNamedCandidateReady $candidate) { return $candidate }
    }
    return $null
}

# ScrollItemPattern is the documented UIA client pattern for bringing an item
# into its container's viewport. One wait owns at most one such interaction;
# it must still observe a fresh, normally-ready element afterward.
function Invoke-UiaSingleOffscreenScroll([object[]]$Candidates, [hashtable]$ScrollState, [scriptblock]$Scroll) {
    if ($null -eq $ScrollState -or $ScrollState["attempted"]) { return $false }
    foreach ($candidate in @($Candidates)) {
        if (-not (Test-UiaNamedCandidateScrollable $candidate)) { continue }
        $ScrollState["attempted"] = $true
        try {
            & $Scroll $candidate["scroll_item"]
            return $true
        } catch {
            $ScrollState["failure"] = $UIA_OFFSCREEN_SCROLL_FAILURE
            return $false
        }
    }
    return $false
}

function Resolve-UiaNamedCandidate([scriptblock]$ReadCandidates, [hashtable]$ScrollState = $null, [scriptblock]$Scroll = $null) {
    $candidates = @(& $ReadCandidates)
    $ready = Find-UiaReadyNamedCandidate $candidates
    if ($null -ne $ready) { return $ready }
    if ($null -eq $Scroll -or -not (Invoke-UiaSingleOffscreenScroll $candidates $ScrollState $Scroll)) { return $null }
    return Find-UiaReadyNamedCandidate @(& $ReadCandidates)
}

function Get-UiaNamedCandidateRecords(
    [Diagnostics.Process]$App,
    [string]$Name,
    [bool]$Actionable = $false,
    [bool]$NeedScrollPattern = $false
) {
    $App.Refresh()
    if ($App.HasExited) { return @() }
    $root = Get-AppAutomationRoot $App
    if ($null -eq $root) { return @() }
    $condition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::NameProperty,
        $Name
    )
    try {
        $candidates = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
    } catch {
        return @()
    }
    $records = @()
    foreach ($match in $candidates) {
        try {
            $bounds = $match.Current.BoundingRectangle
            $enabled = $match.Current.IsEnabled
            $offscreen = $match.Current.IsOffscreen
            $canAct = -not $Actionable -or $match.Current.IsKeyboardFocusable -or @(
                $match.GetSupportedPatterns() | Where-Object {
                    $_ -eq [Windows.Automation.InvokePattern]::Pattern -or
                    $_ -eq [Windows.Automation.SelectionItemPattern]::Pattern
                }
            ).Count -gt 0
            $scrollItem = Get-UiaOptionalScrollItemPattern $NeedScrollPattern $offscreen {
                $pattern = $null
                if ($match.TryGetCurrentPattern([Windows.Automation.ScrollItemPattern]::Pattern, [ref]$pattern)) {
                    return $pattern
                }
                return $null
            }
            $records += [ordered]@{
                element = $match
                enabled = $enabled
                offscreen = $offscreen
                bounds = $bounds
                actionable = $canAct
                scroll_item = $scrollItem
            }
        } catch {
            continue
        }
    }
    return @($records)
}

function Get-UiaNamedElement(
    [Diagnostics.Process]$App,
    [string]$Name,
    [bool]$Actionable = $false,
    [hashtable]$ScrollState = $null
) {
    $scrollOptIn = $null -ne $ScrollState
    $recordReader = { Get-UiaNamedCandidateRecords $App $Name $Actionable $scrollOptIn }
    $record = Resolve-UiaNamedCandidate $recordReader $ScrollState {
        param($ScrollItem)
        ([Windows.Automation.ScrollItemPattern]$ScrollItem).ScrollIntoView()
    }
    if ($null -eq $record) { return $null }
    return $record["element"]
}

function Get-UiaNamedWaitProbeOutcome(
    [string]$Name,
    [scriptblock]$Find,
    [scriptblock]$HasRoot,
    [hashtable]$ScrollState = $null
) {
    $match = & $Find
    if ($null -ne $match) { return New-ProbeReady $match }
    if ($null -ne $ScrollState -and $ScrollState.Contains("failure")) {
        return New-ProbeInvariant $ScrollState["failure"]
    }
    if (-not (& $HasRoot)) { return New-ProbeNotReady "the app has published no native window handle" }
    return New-ProbeNotReady "no enabled, on-screen element is named '$Name'"
}

function Wait-UiaName(
    [Diagnostics.Process]$App,
    [string]$Name,
    [bool]$Actionable = $false,
    [bool]$ScrollOffscreen = $false
) {
    $scrollState = if ($ScrollOffscreen) { @{ attempted = $false } } else { $null }
    return Wait-Readiness "UI state '$Name'" {
        $App.Refresh()
        if ($App.HasExited) { return New-ProbeInvariant "the app exited with code $($App.ExitCode)" }
        return Get-UiaNamedWaitProbeOutcome $Name `
            { Get-UiaNamedElement $App $Name $Actionable $scrollState } `
            { $null -ne (Get-AppAutomationRoot $App) } `
            $scrollState
    } { Get-UiaNamedWaitDiagnostics $App $Name $Actionable } 15000
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
        if ($null -ne (Get-UiaNamedElement $App "Settings" $true)) {
            return New-ProbeReady $true
        }
        $explore = Get-UiaNamedElement $App "Explore first" $true
        if ($null -ne $explore) {
            Invoke-UiaElement $explore "Explore first"
            return New-ProbeNotReady "welcome dismissed"
        }
        return New-ProbeNotReady "neither Explore first nor Settings is on screen"
    } { Get-UiaSummary $App } 20000 | Out-Null
}

function Invoke-UiaNamedControl([Diagnostics.Process]$App, [string]$Name, [string]$ExpectedName) {
    $element = Wait-UiaName $App $Name $true $true
    Invoke-UiaElement $element $Name
    Wait-UiaName $App $ExpectedName $false $true | Out-Null
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

function Open-WindowsPairingEntry([Diagnostics.Process]$App, [string[]]$AllowedNames) {
    $launcher = Wait-UiaName $App "Connect a device" $true
    Invoke-UiaElement $launcher "Connect a device"
    $transition = @{ join_invoked = $false }
    $diagnosticNames = @($AllowedNames) + @("Enter pairing code")
    Wait-Readiness "native pairing entry" {
        try {
            $App.Refresh()
            if ($App.HasExited) { return New-ProbeInvariant "the installed app exited" }
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
            return New-ProbeNotReady "the allowlisted native pairing controls are not ready"
        } catch {
            return New-ProbeTransient "protected accessibility is temporarily unavailable"
        }
    } { Get-ProtectedUiaSummary $App $diagnosticNames } 15000 | Out-Null
}

function Set-UiaScreenshots(
    [Diagnostics.Process]$App,
    [bool]$Allow,
    [string]$CaptureTracePath = ""
) {
    $element = Wait-UiaName $App "Allow screenshots" $true $true
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
    Wait-UiaName $App $ExpectedName $false $true | Out-Null
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
        type = "visual"
        feature = $Feature
        state = $State
        expected_name = $ExpectedName
        screenshot = New-EvidenceFileRecord $EvidenceRoot $screenshot
        accessibility = New-EvidenceFileRecord $EvidenceRoot $accessibility
    }
}

function Write-WindowsFeatureManifest([string]$EvidenceRoot, [object[]]$States) {
    [ordered]@{ schema_version = 2; states = $States } |
        ConvertTo-Json -Depth 8 |
        Set-Content -LiteralPath (Join-Path $EvidenceRoot "feature-states.json") -Encoding utf8
}

function Test-UiaNamedCandidateDiagnostics {
    Assert-True ((Get-UiaDiagnosticBounds ([pscustomobject]@{ X = 1; Y = 2; Width = 0; Height = 44 })) -eq "(1,2,0,44)") "finite UIA candidate bounds were not retained"
    Assert-True ((Get-UiaDiagnosticBounds ([pscustomobject]@{ X = [double]::NaN; Y = 2; Width = 1; Height = 1 })) -eq "non-finite") "non-finite UIA candidate bounds were reported as coordinates"
    $candidateDiagnostic = Format-UiaNamedCandidateDiagnostics "fixture" 12 @(
        [ordered]@{ control_type = "ControlType.CheckBox"; enabled = $false; offscreen = $true; bounds = "(1,2,0,44)"; keyboard_focusable = $true
            patterns = @("invoke", "selection-item", "toggle", "scroll-item", "scroll", "other"); actionable = $true },
        [ordered]@{ status = "stale-or-unreadable" }
    )
    Assert-True ($candidateDiagnostic -match '12 match\(es\); showing 2 of 12' -and $candidateDiagnostic -match 'control_type=ControlType.CheckBox' -and
        $candidateDiagnostic -match 'bounds=\(1,2,0,44\)' -and $candidateDiagnostic -match 'relevant_patterns=invoke,selection-item,toggle,scroll-item,scroll' -and -not ($candidateDiagnostic -match 'other') -and $candidateDiagnostic -match 'candidate\[2\]=stale-or-unreadable') `
        "named UIA candidate diagnostics did not preserve bounded candidate facts"
    $longCandidate = [ordered]@{ control_type = ("x" * $UIA_NAMED_DIAGNOSTIC_MAXIMUM_LENGTH); enabled = $true; offscreen = $false
        bounds = "(0,0,1,1)"; keyboard_focusable = $false; patterns = @(); actionable = $false }
    $boundedDiagnostic = Format-UiaNamedCandidateDiagnostics "fixture" 1 @($longCandidate)
    Assert-True ($boundedDiagnostic.Length -eq $UIA_NAMED_DIAGNOSTIC_MAXIMUM_LENGTH -and $boundedDiagnostic.EndsWith(" [truncated]")) `
        "named UIA candidate diagnostics exceeded their serialized bound"
    $callbackDiagnostics = @(Get-UiaNamedWaitDiagnostics $null "fixture" $true { throw "summary leak" } { throw "candidate leak" })
    Assert-True ($callbackDiagnostics.Count -eq 2 -and $callbackDiagnostics[0] -eq "UIA summary unavailable" -and $callbackDiagnostics[1] -eq "UIA exact-name candidates unavailable") `
        "named UIA wait diagnostics leaked a collection failure"
    $candidateFallback = @(Get-UiaNamedWaitDiagnostics $null "fixture" $true { "summary retained" } { throw "candidate leak" })
    Assert-True ($candidateFallback.Count -eq 2 -and $candidateFallback[0] -eq "summary retained" -and $candidateFallback[1] -eq "UIA exact-name candidates unavailable") `
        "named UIA wait diagnostics lost a surviving summary"
    $candidateSource = ${function:Get-UiaNamedCandidateDiagnostics}.ToString()
    Assert-True ($candidateSource -match '\[Math\]::Min\(\$count, \$UIA_NAMED_DIAGNOSTIC_CANDIDATE_LIMIT\)' -and $candidateSource -match "stale-or-unreadable" -and $candidateSource -match "ScrollItemPattern" -and $candidateSource -match "ScrollPattern" -and
        -not ($candidateSource -match 'Current\.Name|Current\.Value|Current\.AutomationId')) `
        "named UIA candidate diagnostics lost their bounded, path-free failure posture"
    $waitSource = ${function:Wait-UiaName}.ToString()
    Assert-True ($waitSource -match 'Get-UiaNamedWaitDiagnostics \$App \$Name \$Actionable') `
        "named UIA candidate diagnostics are not limited to the named-wait failure path"
    $matchSource = ${function:Get-UiaNamedElement}.ToString()
    Assert-True ($matchSource -match 'Resolve-UiaNamedCandidate \$recordReader \$ScrollState' -and $matchSource -match 'ScrollItemPattern.*ScrollIntoView') `
        "named UIA waits no longer use the documented single-item scroll pattern"
    $readySource = ${function:Test-UiaNamedCandidateReady}.ToString()
    Assert-True ($readySource -match 'Candidate\["enabled"\]' -and $readySource -match '-not \$Candidate\["offscreen"\]' -and
        $readySource -match 'Candidate\["actionable"\]' -and $readySource -match 'Test-UiaFinitePositiveBounds') `
        "named UIA candidate diagnostics changed the existing readiness predicate"
    $recordSource = ${function:Get-UiaNamedCandidateRecords}.ToString()
    Assert-True ($recordSource -match 'Get-UiaOptionalScrollItemPattern \$NeedScrollPattern \$offscreen' -and
        ${function:Get-UiaOptionalScrollItemPattern}.ToString() -match 'Test-UiaScrollItemPatternNeeded \$ScrollOptIn \$Offscreen') `
        "optional ScrollItem reads can discard a visible or protected named candidate"
    $outcomeSource = ${function:Get-UiaNamedWaitProbeOutcome}.ToString()
    Assert-True ($outcomeSource -match 'ScrollState\.Contains\("failure"\)' -and $outcomeSource -match 'New-ProbeInvariant') `
        "a ScrollItem failure no longer fails the named wait closed"
}

function Test-UiaOffscreenNamedNavigation {
    $visible = [ordered]@{
        element = "visible"
        enabled = $true
        offscreen = $false
        bounds = [pscustomobject]@{ X = 4; Y = 5; Width = 20; Height = 21 }
        actionable = $true
        scroll_item = $null
    }
    $offscreen = [ordered]@{
        element = "offscreen"
        enabled = $true
        offscreen = $true
        bounds = [pscustomobject]@{ X = 4; Y = 50; Width = 20; Height = 21 }
        actionable = $true
        scroll_item = "scroll"
    }

    $visibleSeen = @{ reads = 0; scrolls = 0 }
    $visibleResult = Resolve-UiaNamedCandidate {
        $visibleSeen["reads"]++
        return @($visible)
    } @{ attempted = $false } {
        param($ScrollItem)
        $visibleSeen["scrolls"]++
    }
    Assert-True ($visibleResult["element"] -eq "visible" -and $visibleSeen["reads"] -eq 1 -and $visibleSeen["scrolls"] -eq 0) `
        "a visible named UIA target triggered a scroll"
    Assert-True (-not (Test-UiaScrollItemPatternNeeded $false $false) -and -not (Test-UiaScrollItemPatternNeeded $false $true) -and
        -not (Test-UiaScrollItemPatternNeeded $true $false) -and (Test-UiaScrollItemPatternNeeded $true $true)) `
        "ScrollItemPattern lookup was not limited to explicit offscreen navigation"
    $patternReads = @{ count = 0 }
    $visiblePattern = Get-UiaOptionalScrollItemPattern $false $false {
        $patternReads["count"]++
        throw "visible fixture pattern read"
    }
    $protectedPattern = Get-UiaOptionalScrollItemPattern $false $true {
        $patternReads["count"]++
        throw "protected fixture pattern read"
    }
    $failedPattern = Get-UiaOptionalScrollItemPattern $true $true {
        $patternReads["count"]++
        throw "offscreen fixture pattern read"
    }
    Assert-True ($null -eq $visiblePattern -and $null -eq $protectedPattern -and $null -eq $failedPattern -and
        $patternReads["count"] -eq 1 -and (Test-UiaNamedCandidateReady $visible)) `
        "an optional ScrollItem read was attempted outside opt-in offscreen navigation or rejected a ready candidate"

    $afterScroll = [ordered]@{
        element = "after-scroll"
        enabled = $true
        offscreen = $false
        bounds = [pscustomobject]@{ X = 4; Y = 6; Width = 20; Height = 21 }
        actionable = $true
        scroll_item = $null
    }
    $transition = @{ phase = 0; reads = 0; scrolls = 0 }
    $scrollState = @{ attempted = $false }
    $scrolledResult = Resolve-UiaNamedCandidate {
        $transition["reads"]++
        if ($transition["phase"] -eq 0) { return @($offscreen) }
        return @($afterScroll)
    } $scrollState {
        param($ScrollItem)
        Assert-True ($ScrollItem -eq "scroll") "the offscreen target lost its ScrollItemPattern"
        $transition["scrolls"]++
        $transition["phase"] = 1
    }
    Assert-True ($scrolledResult["element"] -eq "after-scroll" -and $transition["reads"] -eq 2 -and $transition["scrolls"] -eq 1 -and $scrollState["attempted"]) `
        "an offscreen named target was not scrolled once then re-read as normally ready"

    $unchangedAfterScroll = @{ calls = 0 }
    $unchangedResult = Resolve-UiaNamedCandidate { return @($offscreen) } @{ attempted = $false } {
        param($ScrollItem)
        $unchangedAfterScroll["calls"]++
    }
    Assert-True ($null -eq $unchangedResult -and $unchangedAfterScroll["calls"] -eq 1) `
        "a target that remained offscreen after ScrollItem was accepted"

    $unacceptable = @(
        @(),
        @($null),
        @([ordered]@{ element = "disabled"; enabled = $false; offscreen = $true; bounds = $offscreen["bounds"]; actionable = $true; scroll_item = "scroll" }),
        @([ordered]@{ element = "no-pattern"; enabled = $true; offscreen = $true; bounds = $offscreen["bounds"]; actionable = $true; scroll_item = $null }),
        @([ordered]@{ element = "not-actionable"; enabled = $true; offscreen = $true; bounds = $offscreen["bounds"]; actionable = $false; scroll_item = "scroll" }),
        @([ordered]@{ element = "zero"; enabled = $true; offscreen = $true; bounds = [pscustomobject]@{ X = 4; Y = 5; Width = 0; Height = 21 }; actionable = $true; scroll_item = "scroll" }),
        @([ordered]@{ element = "non-finite"; enabled = $true; offscreen = $true; bounds = [pscustomobject]@{ X = [double]::NaN; Y = 5; Width = 20; Height = 21 }; actionable = $true; scroll_item = "scroll" })
    )
    foreach ($caseCandidates in $unacceptable) {
        $seen = @{ scrolls = 0 }
        $result = Resolve-UiaNamedCandidate { return $caseCandidates } @{ attempted = $false } {
            param($ScrollItem)
            $seen["scrolls"]++
        }
        Assert-True ($null -eq $result -and $seen["scrolls"] -eq 0) "an absent, stale, disabled, non-actionable, unscrollable, zero, or non-finite target was accepted"
    }

    $failedScrollState = @{ attempted = $false }
    $failedScroll = @{ calls = 0 }
    $failedResult = Resolve-UiaNamedCandidate { return @($offscreen) } $failedScrollState {
        param($ScrollItem)
        $failedScroll["calls"]++
        throw "fixture scroll failure"
    }
    $secondFailedResult = Resolve-UiaNamedCandidate { return @($offscreen) } $failedScrollState {
        param($ScrollItem)
        $failedScroll["calls"]++
    }
    Assert-True ($null -eq $failedResult -and $null -eq $secondFailedResult -and $failedScroll["calls"] -eq 1 -and $failedScrollState["attempted"] -and
        $failedScrollState["failure"] -eq $UIA_OFFSCREEN_SCROLL_FAILURE) `
        "a failed UIA ScrollItem interaction was accepted or repeated"
    $rootProbe = @{ calls = 0 }
    $failureOutcome = Get-UiaNamedWaitProbeOutcome "fixture" { return $null } {
        $rootProbe["calls"]++
        return $true
    } $failedScrollState
    Assert-True ($failureOutcome["kind"] -eq "invariant" -and $failureOutcome["why"] -eq $UIA_OFFSCREEN_SCROLL_FAILURE -and $rootProbe["calls"] -eq 0) `
        "a ScrollItem failure did not stop the named wait before a generic retry"
}

function Test-WindowsUiEvidenceHelpers {
    Test-WindowsUiaClientBootstrapHelpers
    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        Assert-True $script:WindowsUiaClientProviderReady "Windows UIA provider bootstrap did not run before self-test"
        Assert-True ($script:WindowsUiaClientProviderBootstrap["outcome"] -eq "ready") `
            "Windows UIA provider bootstrap did not retain a ready canary receipt"
        Assert-True (Test-WindowsUiaCanaryMappings $script:WindowsUiaClientProviderBootstrap["canary"]["controls"]) `
            "Windows UIA provider bootstrap did not retain the verified canary mapping"
    }
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
    Test-UiaNamedCandidateDiagnostics
    Test-UiaOffscreenNamedNavigation
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
    $protected["display_affinity"] = 17
    Assert-True (Test-WindowProtectedReady $protected) "WDA_EXCLUDEFROMCAPTURE was not accepted as protected"
    $unprotected = [ordered]@{
        foreground = $true; visible = $true; minimized = $false
        capture_allowed = $true; display_affinity = 0
    }
    Assert-True (-not (Test-WindowProtectedReady $unprotected)) "WDA_NONE was accepted as protected"
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
    Test-WindowsProtectedFailureDiagnostics
    $pairingSourceParts = ${function:Open-WindowsPairingEntry}.ToString() -split `
        [regex]::Escape('Invoke-UiaElement $launcher "Connect a device"'), 2
    Assert-True ($pairingSourceParts.Count -eq 2) "native pairing entry launch boundary is missing"
    $afterPairingLaunch = $pairingSourceParts[1]
    Assert-True (-not ($afterPairingLaunch -match "Get-UiaSummary|Wait-UiaName")) `
        "native pairing entry retained raw UIA failure diagnostics after launch"
    $protectedSaver = ${function:Save-WindowsProtectedFeatureState}.ToString()
    Assert-True (-not ($protectedSaver -match "Get-UiaSummary|Wait-UiaName|Assert-UiaSnapshotComplete")) `
        "protected evidence retained raw UIA wait or snapshot diagnostics"
    $caller = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "windows-native-evidence.ps1"))
    $callerParts = $caller -split "Open-WindowsPairingEntry", 2
    Assert-True ($callerParts.Count -eq 2) "Windows evidence lacks the pairing entry caller"
    $closeParts = $callerParts[1] -split 'Write-WindowCaptureObservation \$app \$captureTrace "devices/after-close"', 2
    Assert-True ($closeParts.Count -eq 2) "Windows evidence lacks the protected pairing close boundary"
    Assert-True ($closeParts[0] -match "Close-WindowsProtectedPairingEntry" -and
        -not ($closeParts[0] -match "SendKeys|Wait-UiaName|Get-UiaSummary|Wait-ForegroundWindow")) `
        "Windows pairing caller bypassed the protected close transition"
    Assert-True (-not (${function:Read-ProtectedUiaNode}.ToString() -match "ValuePattern")) `
        "protected accessibility queried ValuePattern"
    $invokeSource = ${function:Invoke-UiaElement}.ToString()
    Assert-True ($invokeSource -match "InvokePattern" -and $invokeSource -match "SelectionItemPattern" -and -not ($invokeSource -match "TogglePattern")) `
        "generic UIA invocation widened into toggle semantics"
    $pairingSource = ${function:Open-WindowsPairingEntry}.ToString()
    Assert-True (-not ($pairingSource -match 'Wait-UiaName\s+\$App\s+"Connect a device"\s+\$true\s+\$true')) `
        "protected pairing entry enabled ordinary offscreen scrolling"
}
