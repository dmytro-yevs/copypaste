$MAX_PROTECTED_FAILURE_NODES = 256

function New-WindowsProtectedFailureDiagnostic(
    [string]$Feature,
    [string]$State,
    [string]$ExpectedName,
    $Snapshot,
    [string[]]$AllowedNames
) {
    Assert-ProtectedUiaSnapshotComplete $Snapshot "$Feature/$State protected failure diagnostic"
    $allNodes = @($Snapshot.nodes)
    $count = [Math]::Min($allNodes.Count, $MAX_PROTECTED_FAILURE_NODES)
    $nodes = @(
        for ($index = 0; $index -lt $count; $index++) {
            $projected = New-ProtectedUiaNode `
                $allNodes[$index]["name"] $allNodes[$index]["control_type"] `
                $allNodes[$index]["enabled"] $allNodes[$index]["offscreen"] `
                $allNodes[$index]["bounds"] $allNodes[$index]["is_password"] $AllowedNames
            [ordered]@{
                index = $index
                name = $projected["name"]
                control_type = $projected["control_type"]
                enabled = $projected["enabled"]
                offscreen = $projected["offscreen"]
                is_password = $projected["is_password"]
                bounds = $projected["bounds"]
            }
        }
    )
    return [ordered]@{
        schema_version = 1
        type = "protected-accessibility-failure"
        feature = $Feature
        state = $State
        expected_name = if ($ExpectedName -in $AllowedNames) { $ExpectedName } else { $null }
        node_read = [ordered]@{
            complete = $true
            read = $allNodes.Count
            included = $nodes.Count
            truncated = $nodes.Count -lt $allNodes.Count
        }
        nodes = $nodes
    }
}

function Save-WindowsProtectedFailureDiagnostic(
    [string]$EvidenceRoot,
    [string]$Feature,
    [string]$State,
    [string]$ExpectedName,
    $Snapshot,
    [string[]]$AllowedNames
) {
    $directory = Join-Path $EvidenceRoot "failure-diagnostics"
    [IO.Directory]::CreateDirectory($directory) | Out-Null
    $relativePath = "failure-diagnostics/protected-accessibility-failure.json"
    $diagnostic = New-WindowsProtectedFailureDiagnostic `
        $Feature $State $ExpectedName $Snapshot $AllowedNames
    $diagnostic | ConvertTo-Json -Depth 8 |
        Set-Content -LiteralPath (Join-Path $EvidenceRoot $relativePath) -Encoding utf8
}

function Assert-WindowsProtectedNodesWithFailureDiagnostic(
    [object[]]$Nodes,
    [string[]]$RequiredNames,
    [string[]]$RequiredEnabledNames,
    [string[]]$RequiredPasswordNames,
    [string]$Context,
    [string]$EvidenceRoot,
    [string]$Feature,
    [string]$State,
    [string]$ExpectedName,
    $Snapshot,
    [string[]]$AllowedNames
) {
    try {
        Assert-WindowsProtectedNodes `
            $Nodes $RequiredNames $RequiredEnabledNames $RequiredPasswordNames $Context
    } catch {
        $assertionFailure = $_
        try {
            Save-WindowsProtectedFailureDiagnostic `
                $EvidenceRoot $Feature $State $ExpectedName $Snapshot $AllowedNames
        } catch {
            $null = $_
        }
        throw $assertionFailure
    }
}
