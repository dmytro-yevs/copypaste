function New-ProtectedUiaNode(
    [AllowNull()][string]$Name,
    [AllowNull()][string]$ControlType,
    [bool]$Enabled,
    [bool]$Offscreen,
    $Bounds,
    [bool]$IsPassword,
    [string[]]$AllowedNames
) {
    return [ordered]@{
        name = if ($Name -in $AllowedNames) { $Name } else { $null }
        control_type = $ControlType
        enabled = $Enabled
        offscreen = $Offscreen
        bounds = $Bounds
        is_password = $IsPassword
    }
}

function Read-ProtectedUiaNode(
    [Windows.Automation.AutomationElement]$Element,
    [string[]]$AllowedNames
) {
    $bounds = $Element.Current.BoundingRectangle
    $coordinates = @($bounds.X, $bounds.Y, $bounds.Width, $bounds.Height)
    $serializedBounds = if (@($coordinates | Where-Object { [double]::IsNaN($_) -or [double]::IsInfinity($_) }).Count -eq 0) {
        [ordered]@{ x = $bounds.X; y = $bounds.Y; width = $bounds.Width; height = $bounds.Height }
    } else {
        $null
    }
    return New-ProtectedUiaNode `
        $Element.Current.Name `
        (Get-UiaControlTypeName $Element.Current.ControlType) `
        $Element.Current.IsEnabled `
        $Element.Current.IsOffscreen `
        $serializedBounds `
        $Element.Current.IsPassword `
        $AllowedNames
}

function Get-ProtectedUiaSnapshot(
    [Windows.Automation.AutomationElement]$Root,
    [string[]]$AllowedNames
) {
    $elements = @($Root) + @($Root.FindAll(
        [Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.Condition]::TrueCondition
    ))
    return Read-UiaSnapshot $elements {
        param($element)
        Read-ProtectedUiaNode $element $AllowedNames
    }
}

function Get-ProtectedUiaSnapshotSummary($Snapshot, [string[]]$AllowedNames) {
    $names = @(
        @($Snapshot.nodes) | ForEach-Object {
            if ($_ -is [Collections.IDictionary] -and $_["name"] -in $AllowedNames) {
                $_["name"]
            }
        } | Sort-Object -Unique
    )
    return "protected UIA: read=$(@($Snapshot.nodes).Count) unreadable=$(@($Snapshot.unreadable).Count) retried=$(@($Snapshot.retried).Count) allowed=$($names -join ',')"
}

function Get-ProtectedUiaSummary([Diagnostics.Process]$App, [string[]]$AllowedNames) {
    try {
        $root = Get-AppAutomationRoot $App
        if ($null -eq $root) { return "protected UIA: native window handle is not ready" }
        return Get-ProtectedUiaSnapshotSummary (Get-ProtectedUiaSnapshot $root $AllowedNames) $AllowedNames
    } catch {
        return "protected UIA: snapshot unavailable"
    }
}

function Assert-ProtectedUiaSnapshotComplete($Snapshot, [string]$Context) {
    $missing = @($Snapshot.unreadable).Count
    if ($missing -gt 0) {
        throw "$Context is a partial protected accessibility snapshot: $missing of $($missing + @($Snapshot.nodes).Count) node(s) could not be read"
    }
    if (@($Snapshot.nodes).Count -eq 0) {
        throw "$Context read no protected accessibility nodes"
    }
}

function Get-ProtectedUiaNamedElement(
    [Diagnostics.Process]$App,
    [string]$Name,
    [bool]$RequireEnabled
) {
    try {
        $App.Refresh()
        if ($App.HasExited) { return $null }
        $root = Get-AppAutomationRoot $App
        if ($null -eq $root) { return $null }
        $condition = [Windows.Automation.PropertyCondition]::new(
            [Windows.Automation.AutomationElement]::NameProperty,
            $Name
        )
        $candidates = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
        foreach ($match in $candidates) {
            $bounds = $match.Current.BoundingRectangle
            if ((-not $RequireEnabled -or $match.Current.IsEnabled) -and
                -not $match.Current.IsOffscreen -and $bounds.Width -gt 0 -and $bounds.Height -gt 0) {
                return $match
            }
        }
    } catch {
        return $null
    }
    return $null
}

function Get-ProtectedUiaNamedRoot([Diagnostics.Process]$App, [string]$Name) {
    try {
        $App.Refresh()
        if ($App.HasExited) { return $null }
        $root = Get-AppAutomationRoot $App
        if ($null -eq $root -or $root.Current.Name -ne $Name) { return $null }
        $bounds = $root.Current.BoundingRectangle
        if (-not $root.Current.IsOffscreen -and $bounds.Width -gt 0 -and $bounds.Height -gt 0) {
            return $root
        }
    } catch {
        return $null
    }
    return $null
}

function Wait-ProtectedUiaName(
    [Diagnostics.Process]$App,
    [string]$Name,
    [string[]]$AllowedNames,
    [bool]$RequireEnabled
) {
    return Wait-Readiness "protected UI state '$Name'" {
        try {
            $App.Refresh()
            if ($App.HasExited) { return New-ProbeInvariant "the installed app exited" }
            $match = Get-ProtectedUiaNamedElement $App $Name $RequireEnabled
            if ($null -ne $match) { return New-ProbeReady $match }
            if ($null -eq (Get-AppAutomationRoot $App)) {
                return New-ProbeNotReady "the protected window handle is not ready"
            }
            return New-ProbeNotReady "the allowlisted protected control is not ready"
        } catch {
            return New-ProbeTransient "protected accessibility is temporarily unavailable"
        }
    } { Get-ProtectedUiaSummary $App $AllowedNames } 15000
}

function Wait-ProtectedUiaRootName(
    [Diagnostics.Process]$App,
    [string]$Name,
    [string[]]$AllowedNames
) {
    return Wait-Readiness "protected root UI state '$Name'" {
        try {
            $App.Refresh()
            if ($App.HasExited) { return New-ProbeInvariant "the installed app exited" }
            $match = Get-ProtectedUiaNamedRoot $App $Name
            if ($null -ne $match) { return New-ProbeReady $match }
            if ($null -eq (Get-AppAutomationRoot $App)) {
                return New-ProbeNotReady "the protected window handle is not ready"
            }
            return New-ProbeNotReady "the allowlisted protected root is not ready"
        } catch {
            return New-ProbeTransient "protected accessibility is temporarily unavailable"
        }
    } { Get-ProtectedUiaSummary $App $AllowedNames } 15000
}

function Test-ProtectedUiaNodeVisible($Node, [string]$Name, [bool]$RequireEnabled) {
    $bounds = $Node["bounds"]
    return $Node["name"] -eq $Name -and
        (-not $RequireEnabled -or $Node["enabled"]) -and
        -not $Node["offscreen"] -and
        $bounds -is [Collections.IDictionary] -and
        $bounds["width"] -gt 0 -and $bounds["height"] -gt 0
}

function Assert-WindowsProtectedNodes(
    [object[]]$Nodes,
    [string[]]$RequiredNames,
    [string[]]$RequiredEnabledNames,
    [string[]]$RequiredPasswordNames,
    [string]$Context
) {
    $root = @($Nodes)[0]
    foreach ($name in $RequiredNames) {
        Assert-True ($null -ne $root -and (Test-ProtectedUiaNodeVisible $root $name $false)) `
            "$Context lacks visible root '$name'"
    }
    $descendants = @($Nodes | Select-Object -Skip 1)
    foreach ($name in $RequiredEnabledNames) {
        $matches = @($descendants | Where-Object { Test-ProtectedUiaNodeVisible $_ $name $true })
        Assert-True ($matches.Count -gt 0) "$Context lacks visible, enabled '$name'"
    }
    foreach ($name in $RequiredPasswordNames) {
        $matches = @($descendants | Where-Object {
            (Test-ProtectedUiaNodeVisible $_ $name $true) -and $_["is_password"]
        })
        Assert-True ($matches.Count -gt 0) "$Context lacks IsPassword=true for '$name'"
    }
}

function New-ProtectedPairingTransitionSummary(
    [IntPtr]$PairingHandle,
    $Window,
    $Snapshot,
    [string[]]$AllowedNames
) {
    $tree = Get-ProtectedUiaSnapshotSummary $Snapshot $AllowedNames
    return "$tree current_is_pairing=$([int64]$Window.handle -eq $PairingHandle.ToInt64()) foreground=$([bool]$Window.foreground) visible=$([bool]$Window.visible) minimized=$([bool]$Window.minimized) capture_allowed=$([bool]$Window.capture_allowed) display_affinity=$([int64]$Window.display_affinity)"
}

function Get-ProtectedPairingTransitionSummary(
    [Diagnostics.Process]$App,
    [IntPtr]$PairingHandle,
    [string[]]$AllowedNames
) {
    try {
        $App.Refresh()
        $window = Get-WindowCaptureState $App.MainWindowHandle
        $root = Get-AppAutomationRoot $App
        if ($null -eq $root) {
            return "protected pairing transition: native window handle is not ready"
        }
        $snapshot = Get-ProtectedUiaSnapshot $root $AllowedNames
        return New-ProtectedPairingTransitionSummary $PairingHandle $window $snapshot $AllowedNames
    } catch {
        return "protected pairing transition: diagnostics unavailable"
    }
}

function Close-WindowsProtectedPairingEntry(
    [Diagnostics.Process]$App,
    [string[]]$AllowedNames
) {
    try {
        $App.Refresh()
        $pairingHandle = $App.MainWindowHandle
        if ($pairingHandle -eq [IntPtr]::Zero) { throw "missing protected handle" }
        [Windows.Forms.SendKeys]::SendWait("{ESC}")
    } catch {
        throw "protected pairing entry could not be dismissed"
    }
    $diagnosticNames = @($AllowedNames) + @("Connect a device")
    return Wait-Readiness "protected pairing entry closed to restored shell" {
        try {
            $App.Refresh()
            if ($App.HasExited) { return New-ProbeInvariant "the installed app exited" }
            $handle = $App.MainWindowHandle
            if ($handle -eq [IntPtr]::Zero) {
                return New-ProbeNotReady "the restored shell handle is not ready"
            }
            if ($handle -eq $pairingHandle) {
                return New-ProbeNotReady "the protected pairing window remains active"
            }
            if ($null -eq (Get-UiaNamedElement $App "Connect a device")) {
                return New-ProbeNotReady "the allowlisted shell control is not ready"
            }
            $window = Get-WindowCaptureState $handle
            $plan = Get-WindowActivationPlan $window
            if ($plan.restore) {
                [CopyPasteNativeWindowEvidence]::ShowWindowAsync($handle, 9) | Out-Null
            }
            if ($plan.activate) {
                [CopyPasteNativeWindowEvidence]::SetForegroundWindow($handle) | Out-Null
            }
            if ($plan.restore -or $plan.activate) {
                $window = Get-WindowCaptureState $handle
            }
            if (Test-WindowCaptureReady $window -and $window.display_affinity -eq 0) {
                return New-ProbeReady $window
            }
            return New-ProbeNotReady "the restored shell window state is not ready"
        } catch {
            return New-ProbeTransient "the protected pairing transition is temporarily unavailable"
        }
    } { Get-ProtectedPairingTransitionSummary $App $pairingHandle $diagnosticNames } 15000
}

function New-WindowsProtectedStateRecord(
    [string]$Feature,
    [string]$State,
    [string]$ExpectedName,
    $Accessibility
) {
    return [ordered]@{
        type = "protected-accessibility"
        feature = $Feature
        state = $State
        expected_name = $ExpectedName
        accessibility = $Accessibility
    }
}

function New-WindowsProtectedAccessibilityDocument(
    [string]$Feature,
    [string]$State,
    [string]$ExpectedName,
    $Window,
    $Snapshot,
    [string[]]$AllowedNames
) {
    $nodes = @($Snapshot.nodes | ForEach-Object {
        New-ProtectedUiaNode `
            $_["name"] $_["control_type"] $_["enabled"] $_["offscreen"] `
            $_["bounds"] $_["is_password"] $AllowedNames
    })
    $retried = @($Snapshot.retried | ForEach-Object {
        [ordered]@{ index = [int]$_.index; attempts = [int]$_.attempts }
    })
    return [ordered]@{
        schema_version = 2
        feature = $Feature
        state = $State
        expected_name = $ExpectedName
        window = $Window
        node_read = [ordered]@{
            complete = $true
            read = $nodes.Count
            retried = $retried
        }
        nodes = $nodes
    }
}

function Save-WindowsProtectedFeatureState(
    [Diagnostics.Process]$App,
    [string]$EvidenceRoot,
    [string]$Feature,
    [string]$State,
    [string]$ExpectedName,
    [string[]]$AllowedNames,
    [string[]]$RequiredNames,
    [string[]]$RequiredEnabledNames,
    [string[]]$RequiredPasswordNames,
    [string]$ArtifactDirectory = "",
    [string]$CaptureTracePath = ""
) {
    foreach ($name in $RequiredNames) {
        Wait-ProtectedUiaRootName $App $name $AllowedNames | Out-Null
    }
    foreach ($name in $RequiredEnabledNames) {
        Wait-ProtectedUiaName $App $name $AllowedNames $true | Out-Null
    }
    $window = Wait-ProtectedForegroundWindow $App
    try {
        Write-WindowCaptureObservation $App $CaptureTracePath "$Feature/$State/protected" $window
    } catch {
        throw "$Feature protected window observation could not be persisted"
    }
    try {
        $snapshot = Get-ProtectedUiaSnapshot (Get-AppAutomationRoot $App) $AllowedNames
    } catch {
        throw "$Feature protected accessibility tree could not be read"
    }
    Assert-ProtectedUiaSnapshotComplete $snapshot "$Feature/$State protected evidence"
    $document = New-WindowsProtectedAccessibilityDocument `
        $Feature $State $ExpectedName $window $snapshot $AllowedNames
    $nodes = @($document.nodes)
    Assert-WindowsProtectedNodes `
        $nodes $RequiredNames $RequiredEnabledNames $RequiredPasswordNames "$Feature protected evidence"
    $relativeDirectory = if ($ArtifactDirectory) { Join-Path $Feature $ArtifactDirectory } else { $Feature }
    $directory = Join-Path $EvidenceRoot $relativeDirectory
    $accessibility = Join-Path $relativeDirectory "accessibility.json"
    try {
        [IO.Directory]::CreateDirectory($directory) | Out-Null
        $document | ConvertTo-Json -Depth 8 |
            Set-Content -LiteralPath (Join-Path $EvidenceRoot $accessibility) -Encoding utf8
        $record = New-EvidenceFileRecord $EvidenceRoot $accessibility
    } catch {
        throw "$Feature protected accessibility evidence could not be persisted"
    }
    return New-WindowsProtectedStateRecord $Feature $State $ExpectedName $record
}
