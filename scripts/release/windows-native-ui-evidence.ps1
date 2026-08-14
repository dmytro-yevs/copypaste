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
        control_type = $Element.Current.ControlType.ProgrammaticName
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

# Diagnostics, so a partial read is described rather than rejected: this runs
# while some other wait has already failed and is the only account of what the
# app was showing.
function Get-UiaSummary([Diagnostics.Process]$App) {
    $root = Get-AppAutomationRoot $App
    if ($null -eq $root) { return "native window handle is not ready" }
    $snapshot = Get-UiaSnapshot $root
    $names = @($snapshot.nodes | Where-Object { $_.name } | Select-Object -First 40 -ExpandProperty name)
    return "UIA names: $($names -join ' | ') [$(Get-UiaSnapshotReport $snapshot)]"
}

function Wait-UiaName([Diagnostics.Process]$App, [string]$Name, [bool]$Actionable = $false) {
    return Wait-Readiness "UI state '$Name'" {
        $App.Refresh()
        if ($App.HasExited) { return New-ProbeInvariant "the app exited with code $($App.ExitCode)" }
        $root = Get-AppAutomationRoot $App
        if ($null -eq $root) { return New-ProbeNotReady "the app has published no native window handle" }
        $condition = [Windows.Automation.PropertyCondition]::new(
            [Windows.Automation.AutomationElement]::NameProperty,
            $Name
        )
        try {
            $candidates = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
        } catch {
            return New-ProbeTransient "the accessibility tree could not be searched: $($_.Exception.Message)"
        }
        $unread = $null
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
                    return New-ProbeReady $match
                }
            } catch {
                $unread = $_.Exception.Message
            }
        }
        if ($unread) { return New-ProbeTransient "an element named '$Name' could not be read: $unread" }
        return New-ProbeNotReady "no enabled, on-screen element is named '$Name'"
    } { Get-UiaSummary $App } 15000
}

function Invoke-UiaNamedControl([Diagnostics.Process]$App, [string]$Name, [string]$ExpectedName) {
    $element = Wait-UiaName $App $Name $true
    $pattern = $null
    if ($element.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        ([Windows.Automation.InvokePattern]$pattern).Invoke()
    } elseif ($element.TryGetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) {
        ([Windows.Automation.SelectionItemPattern]$pattern).Select()
    } elseif ($element.Current.IsKeyboardFocusable) {
        $element.SetFocus()
        [Windows.Forms.SendKeys]::SendWait("{ENTER}")
    } else {
        $supported = @($element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName }) -join ", "
        throw "UI control '$Name' has no actionable accessibility pattern; supported: $supported"
    }
    Wait-UiaName $App $ExpectedName | Out-Null
}

function Set-UiaScreenshots([Diagnostics.Process]$App, [bool]$Allow) {
    $element = Wait-UiaName $App "Allow screenshots" $true
    $pattern = $null
    if (-not $element.TryGetCurrentPattern([Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
        $supported = @($element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName }) -join ", "
        throw "Allow screenshots has no Toggle pattern; supported: $supported"
    }
    $toggle = [Windows.Automation.TogglePattern]$pattern
    $expected = if ($Allow) { [Windows.Automation.ToggleState]::On } else { [Windows.Automation.ToggleState]::Off }
    if ($toggle.Current.ToggleState -ne $expected) { $toggle.Toggle() }
    Wait-Readiness "Allow screenshots=$Allow native state" {
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
    } 15000 | Out-Null
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
    [string]$ArtifactDirectory = ""
) {
    Wait-UiaName $App $ExpectedName | Out-Null
    $root = Get-AppAutomationRoot $App
    $snapshot = Get-UiaSnapshot $root
    Assert-UiaSnapshotComplete $snapshot "$Feature/$State evidence"
    $nodes = $snapshot.nodes
    $markers = @($nodes | Where-Object {
        $_.name -eq $ExpectedName -and $_.enabled -and -not $_.offscreen -and
        $_.bounds.width -gt 0 -and $_.bounds.height -gt 0
    })
    Assert-True ($markers.Count -gt 0) "$Feature evidence lacks a visible, enabled '$ExpectedName' marker"
    $relativeDirectory = if ($ArtifactDirectory) { Join-Path $Feature $ArtifactDirectory } else { $Feature }
    $directory = Join-Path $EvidenceRoot $relativeDirectory
    [IO.Directory]::CreateDirectory($directory) | Out-Null
    $screenshot = Join-Path $relativeDirectory "screenshot.png"
    $accessibility = Join-Path $relativeDirectory "accessibility.json"
    $window = Save-WindowImage $App (Join-Path $EvidenceRoot $screenshot)
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
    Test-WindowsReadinessHelpers
    Test-UiaSnapshotHelpers
    $occluded = [ordered]@{ foreground = $false; visible = $true; minimized = $false; capture_allowed = $true }
    Assert-True (-not (Test-WindowCaptureReady $occluded)) "an occluded window was accepted for capture"
    $protected = [ordered]@{ foreground = $true; visible = $true; minimized = $false; capture_allowed = $false }
    Assert-True (-not (Test-WindowCaptureReady $protected)) "a capture-protected window was accepted for capture"
}
