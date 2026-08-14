# Wait for a readiness condition on a bounded process budget.
#
# The previous loop polled every 100ms for the whole timeout and treated every
# exception as "not ready yet". Both halves were wrong.
#
# A probe that shells out spent one process per 100ms: the installed-app leg's
# 30s wait could launch 300 copies of copypaste.exe, and the daemon-IPC wait
# another 150. The budget here is counted in processes, and the caller declares
# what one probe costs.
#
# `installed app exited with code 1` is not a state that becomes ready — it was
# retried for the full timeout and then reported as a timeout, which reads as a
# slow app rather than a dead one. A probe now returns a typed outcome, and an
# invariant propagates on the attempt that found it.

function New-ProbeReady { param($Value) return [ordered]@{ kind = "ready"; value = $Value } }
function New-ProbeNotReady { param([string]$Why) return [ordered]@{ kind = "not-ready"; why = $Why } }
function New-ProbeTransient { param([string]$Why) return [ordered]@{ kind = "transient"; why = $Why } }
function New-ProbeInvariant { param([string]$Why) return [ordered]@{ kind = "invariant"; why = $Why } }

# Exponential backoff to the deadline, truncated by however many probes the
# process budget pays for. `limited_by` is the half that ran out first, because
# a run that stopped probing early must not report itself as having waited the
# whole timeout.
function Get-ReadinessSchedule {
    param(
        [int]$TimeoutMilliseconds,
        [int]$ProcessBudget = 0,
        [int]$ProcessesPerProbe = 0,
        [int]$FirstDelayMs = 250,
        [int]$MaxDelayMs = 2000
    )
    if ($TimeoutMilliseconds -lt 0) { throw "a readiness timeout cannot be negative" }
    if ($ProcessesPerProbe -lt 0) { throw "a probe cannot cost a negative number of processes" }
    if ($FirstDelayMs -le 0 -or $MaxDelayMs -lt $FirstDelayMs) {
        throw "readiness backoff must start above zero and may not exceed its own cap"
    }

    $maxAttempts = 0
    if ($ProcessesPerProbe -gt 0) {
        if ($ProcessBudget -lt $ProcessesPerProbe) {
            throw "a process budget of $ProcessBudget cannot pay for one probe costing $ProcessesPerProbe"
        }
        $maxAttempts = [Math]::Floor($ProcessBudget / $ProcessesPerProbe)
    }

    $delays = @()
    $elapsed = 0
    $delay = $FirstDelayMs
    while ($elapsed -lt $TimeoutMilliseconds) {
        if ($maxAttempts -gt 0 -and ($delays.Count + 1) -ge $maxAttempts) { break }
        $step = [Math]::Min($delay, $TimeoutMilliseconds - $elapsed)
        $delays += $step
        $elapsed += $step
        $delay = [Math]::Min($MaxDelayMs, $delay * 2)
    }

    $attempts = $delays.Count + 1
    $limitedBy = if ($elapsed -lt $TimeoutMilliseconds) { "process-budget" } else { "timeout" }
    return [ordered]@{
        attempts = $attempts
        delays = @($delays)
        processes = $attempts * $ProcessesPerProbe
        limited_by = $limitedBy
    }
}

function Wait-Readiness {
    param(
        [string]$Description,
        [scriptblock]$Probe,
        [scriptblock]$Diagnostics,
        [int]$TimeoutMilliseconds = 30000,
        [int]$ProcessBudget = 0,
        [int]$ProcessesPerProbe = 0,
        [int]$FirstDelayMs = 250,
        [int]$MaxDelayMs = 2000,
        [scriptblock]$Sleep = { param($ms) Start-Sleep -Milliseconds $ms }
    )
    $schedule = Get-ReadinessSchedule $TimeoutMilliseconds $ProcessBudget $ProcessesPerProbe $FirstDelayMs $MaxDelayMs
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $last = "the probe was never run"
    $transients = 0
    $spent = 0

    for ($attempt = 1; $attempt -le $schedule.attempts; $attempt++) {
        $outcome = & $Probe
        $spent += $ProcessesPerProbe
        # Indexed, not dotted: a probe that returns a bare value must reach the
        # rejection below rather than a strict-mode property error.
        $kind = if ($outcome -is [Collections.IDictionary] -and $outcome.Contains("kind")) { $outcome["kind"] } else { $null }
        if (-not $kind) {
            throw "$Description probe returned no outcome; it must return one of New-ProbeReady, New-ProbeNotReady, New-ProbeTransient or New-ProbeInvariant"
        }
        $why = if ($outcome.Contains("why")) { $outcome["why"] } else { "no reason given" }
        switch ($kind) {
            "ready" { return $outcome["value"] }
            "invariant" {
                throw "$Description cannot become ready: $why. Observed after $attempt probe(s) and $($timer.ElapsedMilliseconds) ms."
            }
            "transient" {
                $transients++
                $last = "transient: $why"
                # Write-Host, not Write-Output: the success stream is this
                # function's return value.
                Write-Host "  readiness: $Description saw a transient failure on probe ${attempt}: $why"
            }
            "not-ready" { $last = "not ready: $why" }
            default { throw "$Description probe returned an unknown outcome kind '$kind'" }
        }
        if ($attempt -lt $schedule.attempts) { & $Sleep $schedule.delays[$attempt - 1] }
    }

    $detail = try { (& $Diagnostics) -join "`n" } catch { $_.Exception.Message }
    $bound = if ($schedule.limited_by -eq "process-budget") {
        "its process budget of $ProcessBudget ran out after $($schedule.attempts) probe(s) costing $ProcessesPerProbe each"
    } else {
        "it timed out after $TimeoutMilliseconds ms and $($schedule.attempts) probe(s)"
    }
    throw ("$Description never became ready: $bound. Spent $spent process(es), saw $transients transient failure(s). " +
           "Last outcome: $last. Diagnostics:`n$detail")
}

function Test-WindowsReadinessHelpers {
    # Before/after, computed rather than asserted from memory: the fixed 100ms
    # poll this replaced ran TimeoutMilliseconds/100 probes.
    foreach ($budget in @(30000, 15000)) {
        $before = $budget / 100
        $after = (Get-ReadinessSchedule $budget 40 1).attempts
        Write-Output "  readiness probes over ${budget}ms: $before before, $after after"
        Assert-True ($after -lt $before / 5) "the backoff schedule did not cut the probe count"
    }

    $schedule = Get-ReadinessSchedule 30000 40 1
    Assert-True ($schedule.limited_by -eq "timeout") "a 40-process budget bound a 30s wait it should have covered"
    Assert-True ($schedule.processes -le 40) "the schedule spends more processes than its budget"

    $starved = Get-ReadinessSchedule 30000 4 1
    Assert-True ($starved.attempts -eq 4) "a 4-process budget did not bound the schedule to 4 probes"
    Assert-True ($starved.limited_by -eq "process-budget") "a budget-bound schedule reported itself as timing out"

    $rejected = $false
    try { Get-ReadinessSchedule 30000 1 2 | Out-Null } catch { $rejected = $_.Exception.Message -match "cannot pay for one probe" }
    Assert-True $rejected "a budget that cannot pay for a single probe was accepted"

    $noSleep = { param($ms) }
    # A hashtable, not a plain counter: assigning to a captured variable inside
    # a scriptblock writes a copy in the block's own scope.
    $seen = @{ probes = 0 }

    $value = Wait-Readiness "a fixture that comes ready" {
        $seen.probes++
        if ($seen.probes -lt 3) { return New-ProbeNotReady "still starting" }
        return New-ProbeReady "observed"
    } { "fixture diagnostics" } 30000 40 1 250 2000 $noSleep
    Assert-True ($value -eq "observed") "a probe that came ready did not return its observation"

    # The defect this file exists for: an invariant must not be retried.
    $seen.probes = 0
    $rejected = $false
    try {
        Wait-Readiness "a fixture that died" {
            $seen.probes++
            return New-ProbeInvariant "installed app exited with code 1"
        } { "fixture diagnostics" } 30000 40 1 250 2000 $noSleep | Out-Null
    } catch {
        $rejected = $_.Exception.Message -match "cannot become ready" -and
                    $_.Exception.Message -match "exited with code 1"
    }
    Assert-True $rejected "an invariant failure was retried instead of propagating"
    Assert-True ($seen.probes -eq 1) "an invariant failure was probed $($seen.probes) times, not once"

    $seen.probes = 0
    $rejected = $false
    try {
        Wait-Readiness "a fixture that never comes ready" {
            $seen.probes++
            return New-ProbeTransient "the CLI refused"
        } { "fixture diagnostics" } 30000 6 2 250 2000 $noSleep | Out-Null
    } catch {
        $rejected = $_.Exception.Message -match "process budget of 6 ran out" -and
                    $_.Exception.Message -match "transient: the CLI refused" -and
                    $_.Exception.Message -match "fixture diagnostics"
    }
    Assert-True $rejected "an exhausted process budget did not name itself, the last outcome and the diagnostics"
    Assert-True ($seen.probes -eq 3) "a 6-process budget at 2 processes per probe ran $($seen.probes) probes, not 3"

    # Narration must not travel on the return channel. It did, and a caller that
    # saw one transient got @(progress line, reply) instead of the reply, so
    # `$status.data` threw PropertyNotFoundException under Set-StrictMode.
    $seen.probes = 0
    $observed = Wait-Readiness "a fixture that recovers from a transient" {
        $seen.probes++
        if ($seen.probes -eq 1) { return New-ProbeTransient "the CLI refused" }
        return New-ProbeReady ([pscustomobject]@{ data = "observed" })
    } { "fixture diagnostics" } 30000 40 1 250 2000 $noSleep
    Assert-True (@($observed).Count -eq 1) "a transient put $(@($observed).Count) values on the ready return"
    Assert-True ($observed.data -eq "observed") "a wait that saw a transient did not return its observation"

    $rejected = $false
    try {
        Wait-Readiness "a fixture with a broken probe" { return "ready" } { "" } 1000 40 1 250 2000 $noSleep | Out-Null
    } catch {
        $rejected = $_.Exception.Message -match "must return one of New-ProbeReady"
    }
    Assert-True $rejected "a probe that returned a bare value was accepted as ready"
}
