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
    $safe = New-ProtectedUiaNode "Pairing code" "ControlType.Edit" $true $false `
        ([ordered]@{ x = 0; y = 0; width = 1; height = 1 }) $true @("Pairing code")
    Assert-True ($safe.name -eq "Pairing code" -and $safe.is_password) `
        "the protected accessibility node lost its safe name or password state"
    $redacted = New-ProtectedUiaNode "secret-value" "ControlType.Edit" $true $false `
        ([ordered]@{ x = 0; y = 0; width = 1; height = 1 }) $true @("Pairing code")
    Assert-True ($null -eq $redacted.name) "protected accessibility retained an unapproved name"
    $unknownControlType = New-ProtectedUiaNode "Pairing code" "ControlType.Secret123" $true $false `
        ([ordered]@{ x = 0; y = 0; width = 1; height = 1 }) $true @("Pairing code")
    Assert-True ($null -eq $unknownControlType.control_type) `
        "protected accessibility retained an unknown control type"
    $pairingNames = @("Add a CopyPaste device", "Pairing code", "Pairing address", "Pair", "Cancel")
    $pairingVisibleNames = @("Add a CopyPaste device")
    $pairingEnabledNames = @("Pairing code", "Pairing address", "Pair", "Cancel")
    $makePairingNodes = {
        param([string]$Missing = "", [string]$Offscreen = "", [string]$Disabled = "")
        foreach ($name in $pairingNames) {
            if ($name -eq $Missing) { continue }
            $enabled = $name -in $pairingEnabledNames -and $name -ne $Disabled
            New-ProtectedUiaNode $name "ControlType.Edit" $enabled ($name -eq $Offscreen) `
                ([ordered]@{ x = 0; y = 0; width = 1; height = 1 }) `
                ($name -in @("Pairing code", "Pairing address")) $pairingNames
        }
    }
    $validPairingNodes = @(& $makePairingNodes)
    Assert-WindowsProtectedNodes $validPairingNodes $pairingVisibleNames $pairingEnabledNames `
        @("Pairing code", "Pairing address") "fixture"
    foreach ($brokenTitle in @("missing", "offscreen")) {
        $rejected = $false
        try {
            $nodes = if ($brokenTitle -eq "missing") {
                @(& $makePairingNodes "Add a CopyPaste device")
            } else {
                @(& $makePairingNodes "" "Add a CopyPaste device")
            }
            Assert-WindowsProtectedNodes $nodes $pairingVisibleNames $pairingEnabledNames `
                @("Pairing code", "Pairing address") "fixture"
        } catch { $rejected = $_.Exception.Message -match "lacks visible root 'Add a CopyPaste device'" }
        Assert-True $rejected "a $brokenTitle protected pairing title was accepted"
    }
    $wrongRoot = New-ProtectedUiaNode $null "ControlType.Window" $true $false `
        ([ordered]@{ x = 0; y = 0; width = 1; height = 1 }) $false $pairingNames
    $titleOnlyInDescendants = @($wrongRoot) + @(& $makePairingNodes)
    $rejected = $false
    try {
        Assert-WindowsProtectedNodes $titleOnlyInDescendants $pairingVisibleNames $pairingEnabledNames `
            @("Pairing code", "Pairing address") "fixture"
    } catch { $rejected = $_.Exception.Message -match "lacks visible root 'Add a CopyPaste device'" }
    Assert-True $rejected "a descendant title was accepted instead of the protected UIA root"
    foreach ($disabledName in $pairingEnabledNames) {
        $rejected = $false
        try {
            $nodes = @(& $makePairingNodes "" "" $disabledName)
            Assert-WindowsProtectedNodes $nodes $pairingVisibleNames $pairingEnabledNames `
                @("Pairing code", "Pairing address") "fixture"
        } catch { $rejected = $_.Exception.Message -match "lacks visible, enabled '$([regex]::Escape($disabledName))'" }
        Assert-True $rejected "disabled protected pairing control '$disabledName' was accepted"
    }
    $record = New-WindowsProtectedStateRecord "devices" "entry" "Pairing code" ([ordered]@{ path = "accessibility.json" })
    Assert-True (-not $record.Contains("screenshot")) "protected state record bound a screenshot"
    $secret = "pairing-secret-0123456789"
    $privatePath = "C:\Users\private\pairing-secret.txt"
    $unsafeSnapshot = [ordered]@{
        nodes = @([ordered]@{
            name = $secret; control_type = "ControlType.Secret123"; enabled = $true
            offscreen = $false; bounds = [ordered]@{
                x = 0; y = 0; width = 1; height = 1
                raw_bounds = [ordered]@{ path = $privatePath }
            }
            is_password = $true; value = "192.0.2.1:48654"; peer_text = "Unverified peer"
        })
        unreadable = @()
        retried = @([ordered]@{ index = 0; attempts = 2; last_failure = $privatePath })
    }
    $summary = Get-ProtectedUiaSnapshotSummary $unsafeSnapshot @("Pairing code")
    $unsafePattern = "pairing-secret|192\.0\.2\.1|Unverified peer|private"
    Assert-True (-not ($summary -match $unsafePattern)) `
        "protected failure diagnostics exposed a secret or local path"
    $document = New-WindowsProtectedAccessibilityDocument "devices" "entry" "Pairing code" `
        $protected $unsafeSnapshot @("Pairing code")
    $serialized = $document | ConvertTo-Json -Depth 8
    Assert-True (-not ($serialized -match $unsafePattern)) `
        "protected accessibility artifact exposed a secret or local path"
    Assert-True (-not ($serialized -match "ControlType\.Secret123")) `
        "protected accessibility artifact retained an unknown control type"
    Assert-True ($null -eq $document.nodes[0].name -and -not $document.node_read.retried[0].Contains("last_failure")) `
        "protected accessibility artifact did not project onto its safe schema"
    Assert-True (-not $document.nodes[0].bounds.Contains("raw_bounds")) `
        "protected accessibility artifact retained an unknown nested bounds field"
    $unknownControlJson = $unknownControlType | ConvertTo-Json -Depth 8
    Assert-True (-not ($unknownControlJson -match "ControlType\.Secret123")) `
        "protected accessibility artifact retained an unknown control type"
    $partialSnapshot = [ordered]@{
        nodes = @([ordered]@{
            name = "Pairing code"; control_type = "ControlType.Secret123"; enabled = $true
            offscreen = $false; bounds = [ordered]@{ x = 0; y = 0; width = 1; height = 1 }
            is_password = $true
        })
        unreadable = @([ordered]@{ index = 1; attempts = 3; reason = $privatePath })
        retried = @()
    }
    $partialRejected = $false
    try {
        New-WindowsProtectedAccessibilityDocument "devices" "entry" "Pairing code" `
            $protected $partialSnapshot @("Pairing code") | Out-Null
    } catch {
        $partialRejected = $_.Exception.Message -match "partial protected accessibility snapshot"
    }
    Assert-True $partialRejected "a partial protected snapshot became an accessibility document"
    $partialDiagnosticRejected = $false
    try {
        New-WindowsProtectedFailureDiagnostic "devices" "entry" "Pairing code" `
            $partialSnapshot @("Pairing code") | Out-Null
    } catch {
        $partialDiagnosticRejected = $_.Exception.Message -match "partial protected accessibility snapshot"
    }
    Assert-True $partialDiagnosticRejected "a partial protected snapshot became a failure diagnostic"
    $failureRoot = Join-Path ([IO.Path]::GetTempPath()) "copypaste-protected-failure-$([guid]::NewGuid())"
    [IO.Directory]::CreateDirectory($failureRoot) | Out-Null
    try {
        $missingPasswordNode = [ordered]@{
            name = "Pairing code"; control_type = "ControlType.Edit"; enabled = $true
            offscreen = $false; bounds = [ordered]@{ x = 0; y = 0; width = 1; height = 1 }
            is_password = $false; value = $secret
        }
        $missingPasswordRoot = [ordered]@{
            name = "Add a CopyPaste device"; control_type = "ControlType.Window"; enabled = $true
            offscreen = $false; bounds = [ordered]@{ x = 0; y = 0; width = 2; height = 2 }
            is_password = $false
        }
        $missingPasswordSnapshot = [ordered]@{ nodes = @($missingPasswordRoot, $missingPasswordNode) }
        $missingPasswordNodes = @(
            New-ProtectedUiaNode `
                $missingPasswordRoot["name"] $missingPasswordRoot["control_type"] `
                $missingPasswordRoot["enabled"] $missingPasswordRoot["offscreen"] `
                $missingPasswordRoot["bounds"] $missingPasswordRoot["is_password"] $pairingNames
            New-ProtectedUiaNode `
                $missingPasswordNode["name"] $missingPasswordNode["control_type"] `
                $missingPasswordNode["enabled"] $missingPasswordNode["offscreen"] `
                $missingPasswordNode["bounds"] $missingPasswordNode["is_password"] $pairingNames
        )
        $assertionFailed = $false
        try {
            Assert-WindowsProtectedNodesWithFailureDiagnostic `
                $missingPasswordNodes @("Add a CopyPaste device") @("Pairing code") @("Pairing code") `
                "fixture protected evidence" $failureRoot "devices" "desktop-pairing-entry" `
                "Pairing code" $missingPasswordSnapshot $pairingNames
        } catch {
            $assertionFailed = $_.Exception.Message -match "lacks IsPassword=true for 'Pairing code'"
        }
        Assert-True $assertionFailed "the missing IsPassword assertion did not remain the original failure"
        $failurePath = Join-Path $failureRoot "failure-diagnostics/protected-accessibility-failure.json"
        Assert-True (Test-Path -LiteralPath $failurePath -PathType Leaf) `
            "the protected UIA failure diagnostic was not retained"
        $failureDocument = Get-Content -Raw -LiteralPath $failurePath | ConvertFrom-Json
        $failureJson = $failureDocument | ConvertTo-Json -Depth 8
        Assert-True ($failureDocument.type -eq "protected-accessibility-failure") `
            "the protected UIA failure diagnostic used the success artifact type"
        Assert-True ($failureDocument.nodes[0].index -eq 0 -and
            $failureDocument.nodes[0].name -eq "Pairing code" -and
            -not $failureDocument.nodes[0].is_password -and
            $failureDocument.nodes[0].bounds.width -eq 1) `
            "the protected UIA failure diagnostic lost its bounded node facts"
        Assert-True (-not ($failureJson -match $unsafePattern) -and
            -not ($failureJson -match "raw_bounds|peer_text|value|last_failure|reason")) `
            "the protected UIA failure diagnostic retained raw node fields"
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $failureRoot "devices/desktop-pairing-entry/accessibility.json"))) `
            "a protected UIA failure emitted a successful accessibility receipt"
        $largeSnapshot = [ordered]@{
            nodes = @(0..$MAX_PROTECTED_FAILURE_NODES | ForEach-Object {
                [ordered]@{
                    name = "Pairing code"; control_type = "ControlType.Edit"; enabled = $true
                    offscreen = $false; bounds = [ordered]@{ x = 0; y = 0; width = 1; height = 1 }
                    is_password = $true
                }
            })
            unreadable = @()
            retried = @()
        }
        $bounded = New-WindowsProtectedFailureDiagnostic `
            "devices" "desktop-pairing-entry" "Pairing code" $largeSnapshot $pairingNames
        Assert-True ($bounded.nodes.Count -eq $MAX_PROTECTED_FAILURE_NODES -and $bounded.node_read.truncated) `
            "the protected UIA failure diagnostic was not bounded"
    } finally {
        Remove-Item -LiteralPath $failureRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
    $failureText = try {
        Wait-Readiness "protected fixture" { New-ProbeNotReady "allowlisted controls not ready" } `
            { Get-ProtectedUiaSnapshotSummary $unsafeSnapshot @("Pairing code") } 0 | Out-Null
    } catch { $_.Exception.Message }
    Assert-True ($failureText -match "protected UIA" -and -not ($failureText -match $unsafePattern)) `
        "protected wait failure exposed a secret or local path"
    $modalWindow = [ordered]@{
        handle = 41; foreground = $true; visible = $true; minimized = $false
        capture_allowed = $false; display_affinity = 17
    }
    $transitionFailure = try {
        Wait-Readiness "protected close fixture" {
            New-ProbeNotReady "the protected pairing window remains active"
        } {
            New-ProtectedPairingTransitionSummary ([IntPtr]41) $modalWindow $unsafeSnapshot @("Pairing code")
        } 0 | Out-Null
    } catch { $_.Exception.Message }
    Assert-True ($transitionFailure -match "current_is_pairing=True" -and -not ($transitionFailure -match $unsafePattern)) `
        "failed protected pairing dismissal exposed a secret or local path"
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
}
