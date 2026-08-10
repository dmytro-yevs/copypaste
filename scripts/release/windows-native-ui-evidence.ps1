$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Wait-Observed(
    [string]$Description,
    [scriptblock]$Probe,
    [scriptblock]$Diagnostics,
    [int]$TimeoutMilliseconds = 15000,
    [int]$PollMilliseconds = 100
) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $lastFailure = "probe returned no ready state"
    while ($timer.ElapsedMilliseconds -lt $TimeoutMilliseconds) {
        try {
            $value = & $Probe
            if ($null -ne $value -and $value -ne $false) { return $value }
        } catch {
            $lastFailure = $_.Exception.Message
        }
        $remaining = $TimeoutMilliseconds - $timer.ElapsedMilliseconds
        if ($remaining -gt 0) {
            Start-Sleep -Milliseconds ([Math]::Min($PollMilliseconds, $remaining))
        }
    }
    $detail = try { (& $Diagnostics) -join "`n" } catch { $_.Exception.Message }
    throw "$Description timed out after $TimeoutMilliseconds ms. Last probe: $lastFailure. Diagnostics:`n$detail"
}
. (Join-Path $PSScriptRoot "windows-native-window-evidence.ps1")

function Get-AppAutomationRoot([Diagnostics.Process]$App) {
    $App.Refresh()
    if ($App.HasExited) { throw "installed app exited with code $($App.ExitCode)" }
    if ($App.MainWindowHandle -eq 0) { return $null }
    return [Windows.Automation.AutomationElement]::FromHandle($App.MainWindowHandle)
}

function Get-UiaNodes([Windows.Automation.AutomationElement]$Root) {
    $elements = @($Root) + @($Root.FindAll(
        [Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.Condition]::TrueCondition
    ))
    return @($elements | ForEach-Object {
        try {
            $bounds = $_.Current.BoundingRectangle
            $coordinates = @($bounds.X, $bounds.Y, $bounds.Width, $bounds.Height)
            $serializedBounds = if (@($coordinates | Where-Object { [double]::IsNaN($_) -or [double]::IsInfinity($_) }).Count -eq 0) {
                [ordered]@{ x = $bounds.X; y = $bounds.Y; width = $bounds.Width; height = $bounds.Height }
            } else {
                $null
            }
            [ordered]@{
                name = $_.Current.Name
                control_type = $_.Current.ControlType.ProgrammaticName
                automation_id = $_.Current.AutomationId
                enabled = $_.Current.IsEnabled
                offscreen = $_.Current.IsOffscreen
                bounds = $serializedBounds
            }
        } catch {}
    })
}

function Get-UiaSummary([Diagnostics.Process]$App) {
    $root = Get-AppAutomationRoot $App
    if ($null -eq $root) { return "native window handle is not ready" }
    $names = @(Get-UiaNodes $root | Where-Object { $_.name } | Select-Object -First 40 -ExpandProperty name)
    return "UIA names: $($names -join ' | ')"
}

function Wait-UiaName([Diagnostics.Process]$App, [string]$Name, [bool]$Actionable = $false) {
    return Wait-Observed "UI state '$Name'" {
        $root = Get-AppAutomationRoot $App
        if ($null -eq $root) { return $null }
        $condition = [Windows.Automation.PropertyCondition]::new(
            [Windows.Automation.AutomationElement]::NameProperty,
            $Name
        )
        $matches = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
        foreach ($match in $matches) {
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
            } catch {}
        }
        return $null
    } { Get-UiaSummary $App }
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
    Wait-Observed "Allow screenshots=$Allow native state" {
        if ($toggle.Current.ToggleState -ne $expected) { return $null }
        $App.Refresh()
        $state = Get-WindowCaptureState $App.MainWindowHandle
        if (($Allow -and $state.capture_allowed) -or (-not $Allow -and $state.display_affinity -gt 0)) { return $state }
        return $null
    } {
        $state = Get-WindowCaptureState $App.MainWindowHandle
        @(Get-UiaSummary $App; "display affinity=$($state.display_affinity)")
    } | Out-Null
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
    $nodes = Get-UiaNodes $root
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
        schema_version = 1
        feature = $Feature
        state = $State
        expected_name = $ExpectedName
        window = $window
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
    $rejected = $false
    try {
        Wait-Observed "fixture readiness" { $null } { "fixture diagnostics" } 20 5 | Out-Null
    } catch {
        $rejected = $_.Exception.Message -match "timed out" -and $_.Exception.Message -match "fixture diagnostics"
    }
    Assert-True $rejected "readiness timeout omitted its observable diagnostics"
    $occluded = [ordered]@{ foreground = $false; visible = $true; minimized = $false; capture_allowed = $true }
    Assert-True (-not (Test-WindowCaptureReady $occluded)) "an occluded window was accepted for capture"
    $protected = [ordered]@{ foreground = $true; visible = $true; minimized = $false; capture_allowed = $false }
    Assert-True (-not (Test-WindowCaptureReady $protected)) "a capture-protected window was accepted for capture"
}
