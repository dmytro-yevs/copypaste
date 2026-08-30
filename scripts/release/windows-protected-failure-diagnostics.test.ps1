param(
    [switch]$RunProtectedFixtureTests
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "windows-readiness-lib.ps1")
if ($null -eq (Get-Command New-ProtectedUiaNode -ErrorAction SilentlyContinue)) {
    . (Join-Path $PSScriptRoot "windows-native-protected-evidence.ps1")
}
if ($null -eq (Get-Command New-WindowsProtectedFailureDiagnostic -ErrorAction SilentlyContinue)) {
    . (Join-Path $PSScriptRoot "windows-protected-failure-diagnostics.ps1")
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Test-WindowsProtectedFailureDiagnostics {
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
    $protectedWindow = [ordered]@{
        foreground = $true; visible = $true; minimized = $false
        capture_allowed = $false; display_affinity = 17
    }
    $document = New-WindowsProtectedAccessibilityDocument "devices" "entry" "Pairing code" `
        $protectedWindow $unsafeSnapshot @("Pairing code")
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
            $protectedWindow $partialSnapshot @("Pairing code") | Out-Null
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
    $missingFieldSnapshot = [ordered]@{
        nodes = $partialSnapshot.nodes
    }
    $missingFieldRoot = Join-Path ([IO.Path]::GetTempPath()) "copypaste-protected-missing-field-$([guid]::NewGuid())"
    [IO.Directory]::CreateDirectory($missingFieldRoot) | Out-Null
    try {
        $missingFieldRejected = $false
        try {
            Save-WindowsProtectedFailureDiagnostic `
                $missingFieldRoot "devices" "entry" "Pairing code" $missingFieldSnapshot @("Pairing code")
        } catch {
            $missingFieldRejected = $true
        }
        Assert-True $missingFieldRejected "a snapshot missing completeness fields was accepted"
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $missingFieldRoot `
            "failure-diagnostics/protected-accessibility-failure.json"))) `
            "a snapshot missing completeness fields emitted a diagnostic"
    } finally {
        Remove-Item -LiteralPath $missingFieldRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
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
        $missingPasswordSnapshot = [ordered]@{
            nodes = @($missingPasswordRoot, $missingPasswordNode)
            unreadable = @()
            retried = @()
        }
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
            $failureDocument.nodes[0].name -eq "Add a CopyPaste device" -and
            $failureDocument.nodes[0].control_type -eq "ControlType.Window" -and
            $failureDocument.nodes[1].index -eq 1 -and
            $failureDocument.nodes[1].name -eq "Pairing code" -and
            $failureDocument.nodes[1].control_type -eq "ControlType.Edit" -and
            -not $failureDocument.nodes[1].is_password -and
            $failureDocument.nodes[1].bounds.width -eq 1) `
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
}

if ($RunProtectedFixtureTests) {
    Test-WindowsProtectedFailureDiagnostics
    Write-Output "PASS: protected UIA diagnostics fixture contracts"
}
