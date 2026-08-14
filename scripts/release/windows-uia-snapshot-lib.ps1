# Read a UIA accessibility snapshot, or say which nodes could not be read.
#
# Reading one node's properties raises while the tree changes under the walk —
# ElementNotAvailable when the element went away, or a COM failure from the
# provider. The walk used to wrap each read in `catch {}`, so a node that could
# not be read was dropped and the snapshot came back one node shorter with
# nothing recording that. A marker that was never read then looked exactly like
# a marker the app never showed, and the evidence file claimed to be the app's
# accessibility tree while being a subset of it.
#
# A transient read is retried and reported. A read that never succeeds makes the
# snapshot partial, and a partial snapshot is not evidence.

$UIA_NODE_READ_ATTEMPTS = 3

# `Read` is applied to each element and returns that element's record. Passing
# it in is what lets the self-test drive this with elements that fail on demand.
function Read-UiaSnapshot {
    param(
        [object[]]$Elements,
        [scriptblock]$Read,
        [int]$Attempts = $UIA_NODE_READ_ATTEMPTS
    )
    $nodes = @()
    $unreadable = @()
    $retried = @()
    $index = -1
    foreach ($element in @($Elements)) {
        $index++
        $record = $null
        $held = $false
        $failure = "the node reader returned nothing"
        $used = 0
        for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
            $used = $attempt
            try {
                $record = & $Read $element
                $held = $true
                break
            } catch {
                $failure = $_.Exception.Message
            }
        }
        if ($held) {
            if ($used -gt 1) {
                $retried += [ordered]@{ index = $index; attempts = $used; last_failure = $failure }
            }
            $nodes += , $record
        } else {
            $unreadable += [ordered]@{ index = $index; attempts = $used; reason = $failure }
        }
    }
    return [ordered]@{
        nodes = @($nodes)
        unreadable = @($unreadable)
        retried = @($retried)
    }
}

function Get-UiaSnapshotReport($Snapshot) {
    $parts = @("$(@($Snapshot.nodes).Count) node(s) read")
    foreach ($entry in @($Snapshot.retried)) {
        $parts += "node $($entry.index) read after $($entry.attempts) attempts: $($entry.last_failure)"
    }
    foreach ($entry in @($Snapshot.unreadable)) {
        $parts += "node $($entry.index) unreadable after $($entry.attempts) attempts: $($entry.reason)"
    }
    return $parts -join "; "
}

function Assert-UiaSnapshotComplete($Snapshot, [string]$Context) {
    $missing = @($Snapshot.unreadable).Count
    if ($missing -gt 0) {
        throw "$Context is a partial accessibility snapshot: $missing of $($missing + @($Snapshot.nodes).Count) node(s) could not be read. $(Get-UiaSnapshotReport $Snapshot)"
    }
    if (@($Snapshot.nodes).Count -eq 0) {
        throw "$Context read no accessibility nodes at all"
    }
}

function Test-UiaSnapshotHelpers {
    $rejected = $false
    $flaky = @{}
    # Fails twice then succeeds: the read the retry exists for.
    $reader = {
        param($element)
        $flaky[$element] = 1 + [int]$flaky[$element]
        if ($element -eq "flaky" -and $flaky[$element] -lt 3) { throw "ElementNotAvailable" }
        if ($element -eq "gone") { throw "the element is no longer available" }
        return [ordered]@{ name = $element }
    }

    $clean = Read-UiaSnapshot @("a", "b") $reader
    Assert-True (@($clean.nodes).Count -eq 2) "a readable tree did not produce every node"
    Assert-True (@($clean.unreadable).Count -eq 0) "a readable tree reported unreadable nodes"
    Assert-UiaSnapshotComplete $clean "the readable fixture"

    $flaky.Clear()
    $recovered = Read-UiaSnapshot @("a", "flaky") $reader
    Assert-True (@($recovered.nodes).Count -eq 2) "a transient node read was not retried"
    Assert-True (@($recovered.retried).Count -eq 1) "a retried node read went unreported"
    Assert-True ((Get-UiaSnapshotReport $recovered) -match "ElementNotAvailable") `
        "the retried node's failure is missing from the report"
    Assert-UiaSnapshotComplete $recovered "the recovered fixture"

    $partial = Read-UiaSnapshot @("a", "gone") $reader
    Assert-True (@($partial.nodes).Count -eq 1) "an unreadable node was counted as read"
    Assert-True (@($partial.unreadable).Count -eq 1) "an unreadable node was dropped silently"
    try {
        Assert-UiaSnapshotComplete $partial "the partial fixture"
    } catch {
        $rejected = $_.Exception.Message -match "partial accessibility snapshot" -and
                    $_.Exception.Message -match "no longer available"
    }
    Assert-True $rejected "a partial accessibility snapshot was accepted as evidence"

    $rejected = $false
    try {
        Assert-UiaSnapshotComplete (Read-UiaSnapshot @() $reader) "the empty fixture"
    } catch {
        $rejected = $_.Exception.Message -match "no accessibility nodes"
    }
    Assert-True $rejected "an empty accessibility snapshot was accepted as evidence"
}
