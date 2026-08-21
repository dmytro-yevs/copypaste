$script:WindowsProcessTraceLimit = 128
$script:WindowsProcessNames = @("copypaste-ui.exe", "copypaste-daemon.exe")
$script:WindowsProcessTraceCleanupMilliseconds = 2000

function Clear-WindowsProcessTraceSubscriptions([array]$SourceIds) {
    foreach ($sourceId in $SourceIds) {
        Unregister-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue
    }
    $wait = [Diagnostics.Stopwatch]::StartNew()
    do {
        foreach ($sourceId in $SourceIds) {
            Get-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue |
                Remove-Event -ErrorAction SilentlyContinue
        }
        $subscribers = @($SourceIds | Where-Object {
            @(Get-EventSubscriber -SourceIdentifier $_ -ErrorAction SilentlyContinue).Count -ne 0
        })
        $events = @($SourceIds | Where-Object {
            @(Get-Event -SourceIdentifier $_ -ErrorAction SilentlyContinue).Count -ne 0
        })
        if ($subscribers.Count -eq 0 -and $events.Count -eq 0) { return }
        Start-Sleep -Milliseconds 25
    } while ($wait.ElapsedMilliseconds -lt $script:WindowsProcessTraceCleanupMilliseconds)
    throw "process trace cleanup did not settle"
}

function ConvertTo-WindowsProcessTraceLine([string]$Kind, $Record, $OwnedByPid = @{}) {
    $name = [IO.Path]::GetFileName(([string]$Record.ProcessName)).ToLowerInvariant()
    $processId = [uint32]$Record.ProcessID
    $time = [uint64]$Record.TIME_CREATED
    if ($Kind -eq "start") {
        if ($name -notin $script:WindowsProcessNames) { return $null }
        $OwnedByPid[$processId] = [pscustomobject]@{ Time = $time; Name = $name }
    } else {
        if ($name -notin $script:WindowsProcessNames -and $OwnedByPid.ContainsKey($processId) -and
            $OwnedByPid[$processId].Time -le $time) {
            $name = $OwnedByPid[$processId].Name
        }
        $OwnedByPid.Remove($processId)
    }
    if ($name -notin $script:WindowsProcessNames) { return $null }
    $line = [ordered]@{
        time_100ns = $time
        kind = $Kind
        executable = $name
        pid = $processId
        parent_pid = [uint32]$Record.ParentProcessID
    }
    if ($Kind -eq "stop") {
        $line.exit_code = [uint32]$Record.ExitStatus
    }
    return $line | ConvertTo-Json -Compress
}

function Start-WindowsProcessTrace(
    [string]$Path,
    [ValidateRange(1, 128)]
    [int]$Limit = $script:WindowsProcessTraceLimit
) {
    $token = [guid]::NewGuid().ToString("N")
    $startId = "CopyPasteProcessStart-$token"
    $stopId = "CopyPasteProcessStop-$token"
    $filter = "ProcessName = 'copypaste-ui.exe' OR ProcessName = 'copypaste-daemon.exe'"
    try {
        Register-CimIndicationEvent -Namespace root/cimv2 `
            -Query "SELECT * FROM Win32_ProcessStartTrace WHERE $filter" `
            -SourceIdentifier $startId | Out-Null
        Register-CimIndicationEvent -Namespace root/cimv2 `
            -Query "SELECT * FROM Win32_ProcessStopTrace" `
            -SourceIdentifier $stopId | Out-Null
        return [pscustomobject]@{
            Path = $Path
            StartId = $startId
            StopId = $stopId
            Available = $true
            Limit = $Limit
        }
    } catch {
        Clear-WindowsProcessTraceSubscriptions @($startId, $stopId)
        '{"kind":"trace-unavailable","reason":"subscription-failed"}' |
            Set-Content -LiteralPath $Path -Encoding utf8
        return [pscustomobject]@{
            Path = $Path
            StartId = $startId
            StopId = $stopId
            Available = $false
            Limit = $Limit
        }
    }
}

function Write-WindowsProcessTrace([array]$Records, $Trace) {
    $ownedByPid = @{}
    $seen = @{}
    $position = 0
    $converted = @(
        $Records |
            ForEach-Object {
                $sequenceProperty = $_.PSObject.Properties["Sequence"]
                [pscustomobject]@{
                    Kind = $_.Kind
                    Record = $_.Record
                    Sequence = if ($null -ne $sequenceProperty) {
                        [uint64]$sequenceProperty.Value
                    } else { [uint64]::MaxValue }
                    Boundary = if ($_.Kind -eq "stop") { 0 } else { 1 }
                    Position = $position++
                }
            } |
            Sort-Object { [uint64]$_.Record.TIME_CREATED }, Sequence, Boundary, Position |
            Where-Object {
                $record = $_.Record
                $identity = if ($_.Sequence -ne [uint64]::MaxValue) {
                    "sequence:$($_.Sequence)"
                } else {
                    "fields"
                }
                $key = "{0}|{1}|{2}|{3}|{4}|{5}|{6}" -f $identity, $_.Kind,
                    ([uint64]$record.TIME_CREATED), ([uint32]$record.ProcessID),
                    ([string]$record.ProcessName).ToLowerInvariant(),
                    ([uint32]$record.ParentProcessID), ([uint32]$record.ExitStatus)
                if ($seen.ContainsKey($key)) { return $false }
                $seen[$key] = $true
                return $true
            } |
            ForEach-Object { ConvertTo-WindowsProcessTraceLine $_.Kind $_.Record $ownedByPid } |
            Where-Object { $null -ne $_ }
    )
    $lines = @($converted | Select-Object -First $Trace.Limit)
    if ($converted.Count -gt $Trace.Limit) {
        $lines += ([ordered]@{ kind = "trace-truncated"; limit = $Trace.Limit } |
            ConvertTo-Json -Compress)
    }
    if ($lines.Count -eq 0) { $lines = @('{"kind":"trace-empty"}') }
    $lines | Set-Content -LiteralPath $Trace.Path -Encoding utf8
}

function Stop-WindowsProcessTrace($Trace, [scriptblock]$ShutdownActivity = $null) {
    if ($null -eq $Trace -or -not $Trace.Available) { return }
    try {
        Start-Sleep -Milliseconds 300
        foreach ($sourceId in @($Trace.StartId, $Trace.StopId)) {
            Unregister-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue
        }
        if ($null -ne $ShutdownActivity) { & $ShutdownActivity }
        $records = @()
        foreach ($source in @(
            @{ Id = $Trace.StartId; Kind = "start" },
            @{ Id = $Trace.StopId; Kind = "stop" }
        )) {
            foreach ($event in @(Get-Event -SourceIdentifier $source.Id -ErrorAction SilentlyContinue)) {
                $records += [pscustomobject]@{
                    Kind = $source.Kind
                    Record = $event.SourceEventArgs.NewEvent
                    Sequence = [uint64]$event.EventIdentifier
                }
            }
        }
        Write-WindowsProcessTrace $records $Trace
    } finally {
        Clear-WindowsProcessTraceSubscriptions @($Trace.StartId, $Trace.StopId)
    }
}

function Test-WindowsProcessTraceHelpers {
    $start = ConvertTo-WindowsProcessTraceLine "start" ([pscustomobject]@{
        TIME_CREATED = 100
        ProcessName = "copypaste-ui.exe"
        ProcessID = 41
        ParentProcessID = 7
    }) | ConvertFrom-Json
    if ($start.kind -ne "start" -or $start.executable -ne "copypaste-ui.exe" -or
        $start.pid -ne 41 -or $start.parent_pid -ne 7) {
        throw "process trace start records lost process identity"
    }

    $ownedByPid = @{}
    $ownedByPid[[uint32]42] = [pscustomobject]@{ Time = [uint64]100; Name = "copypaste-daemon.exe" }
    $stop = ConvertTo-WindowsProcessTraceLine "stop" ([pscustomobject]@{
        TIME_CREATED = 200
        ProcessName = "copypaste-daem"
        ProcessID = 42
        ParentProcessID = 41
        ExitStatus = [uint32]3221225477
    }) $ownedByPid | ConvertFrom-Json
    if ($stop.kind -ne "stop" -or $stop.executable -ne "copypaste-daemon.exe" -or
        $stop.exit_code -ne 3221225477) {
        throw "process trace stop records lost the exit code"
    }

    $reused = ConvertTo-WindowsProcessTraceLine "stop" ([pscustomobject]@{
        TIME_CREATED = 50
        ProcessName = "unrelated.exe"
        ProcessID = 42
        ParentProcessID = 1
        ExitStatus = 0
    }) $ownedByPid
    if ($null -ne $reused) { throw "process trace correlated a stop before its target start" }

    $ownedByPid[[uint32]42] = [pscustomobject]@{ Time = [uint64]100; Name = "copypaste-daemon.exe" }
    $null = ConvertTo-WindowsProcessTraceLine "stop" ([pscustomobject]@{
        TIME_CREATED = 200
        ProcessName = "copypaste-daem"
        ProcessID = 42
        ParentProcessID = 41
        ExitStatus = 0
    }) $ownedByPid
    $pidReuse = ConvertTo-WindowsProcessTraceLine "stop" ([pscustomobject]@{
        TIME_CREATED = 300
        ProcessName = "unrelated.exe"
        ProcessID = 42
        ParentProcessID = 1
        ExitStatus = 0
    }) $ownedByPid
    if ($null -ne $pidReuse) { throw "process trace retained ownership after a matching stop" }

    $ignored = ConvertTo-WindowsProcessTraceLine "start" ([pscustomobject]@{
        TIME_CREATED = 300
        ProcessName = "private-secret.exe"
        ProcessID = 43
        ParentProcessID = 7
    })
    if ($null -ne $ignored) {
        throw "process trace accepted an unrelated executable identity"
    }

    $fixturePath = Join-Path ([IO.Path]::GetTempPath()) "copypaste-process-trace-fixture-$([guid]::NewGuid()).jsonl"
    try {
        $startRecord = [pscustomobject]@{ TIME_CREATED = 100; ProcessName = "copypaste-ui.exe";
            ProcessID = 51; ParentProcessID = 7; ExitStatus = 0 }
        $stopRecord = [pscustomobject]@{ TIME_CREATED = 200; ProcessName = "copypaste-ui";
            ProcessID = 51; ParentProcessID = 7; ExitStatus = 9 }
        $reuseRecord = [pscustomobject]@{ TIME_CREATED = 200; ProcessName = "copypaste-daemon.exe";
            ProcessID = 51; ParentProcessID = 1; ExitStatus = 0 }
        $reuseStop = [pscustomobject]@{ TIME_CREATED = 300; ProcessName = "unrelated.exe";
            ProcessID = 51; ParentProcessID = 1; ExitStatus = 4 }
        $secondStart = [pscustomobject]@{ TIME_CREATED = 400; ProcessName = "copypaste-daemon.exe";
            ProcessID = 52; ParentProcessID = 7; ExitStatus = 0 }
        $records = @(
            [pscustomobject]@{ Kind = "stop"; Record = $reuseStop; Sequence = 4 },
            [pscustomobject]@{ Kind = "start"; Record = $reuseRecord; Sequence = 3 },
            [pscustomobject]@{ Kind = "stop"; Record = $stopRecord; Sequence = 2 },
            [pscustomobject]@{ Kind = "start"; Record = $startRecord; Sequence = 1 },
            [pscustomobject]@{ Kind = "start"; Record = $startRecord; Sequence = 1 },
            [pscustomobject]@{ Kind = "stop"; Record = $stopRecord; Sequence = 2 },
            [pscustomobject]@{ Kind = "start"; Record = $secondStart; Sequence = 5 }
        )
        Write-WindowsProcessTrace $records ([pscustomobject]@{ Path = $fixturePath; Limit = 3 })
        $fixture = @(Get-Content -LiteralPath $fixturePath | ForEach-Object { $_ | ConvertFrom-Json })
        $events = @($fixture | Where-Object { $_.kind -in @("start", "stop") })
        if ($events.Count -ne 3 -or $events[0].kind -ne "start" -or
            $events[1].kind -ne "stop" -or $events[2].kind -ne "start") {
            throw "process trace did not reorder and deduplicate causal events before truncation"
        }
        if (@($fixture | Where-Object { $_.kind -eq "trace-truncated" }).Count -ne 1) {
            throw "process trace fixture did not prove the unique-record bound"
        }
        if ($events[0].executable -ne "copypaste-ui.exe" -or
            $events[1].executable -ne "copypaste-ui.exe" -or
            $events[1].exit_code -ne 9 -or
            $events[2].executable -ne "copypaste-daemon.exe") {
            throw "process trace fixture attributed a reused PID"
        }
        if (($fixture | ConvertTo-Json -Compress) -match "unrelated|private-secret") {
            throw "process trace fixture disclosed an unrelated executable"
        }
    } finally {
        Remove-Item -LiteralPath $fixturePath -Force -ErrorAction SilentlyContinue
    }
}

function Test-WindowsProcessTraceCollector {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { return }
    $root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-process-trace-$([guid]::NewGuid())"
    [IO.Directory]::CreateDirectory($root) | Out-Null
    $trace = $null
    try {
        $ui = Join-Path $root "copypaste-ui.exe"
        $daemon = Join-Path $root "copypaste-daemon.exe"
        Copy-Item -LiteralPath $env:ComSpec -Destination $ui
        Copy-Item -LiteralPath $env:ComSpec -Destination $daemon
        $trace = Start-WindowsProcessTrace (Join-Path $root "trace.jsonl") 3
        if (-not $trace.Available) { throw "real process trace subscription was unavailable" }
        foreach ($sourceId in @($trace.StartId, $trace.StopId)) {
            if (@(Get-EventSubscriber -SourceIdentifier $sourceId).Count -ne 1) {
                throw "real process trace subscription was not registered"
            }
        }
        $fixturePids = @()
        foreach ($executable in @($ui, $daemon)) {
            $process = Start-Process -FilePath $executable `
                -ArgumentList "/d", "/c", "exit 23" -Wait -PassThru
            if ($process.ExitCode -ne 23) { throw "process trace fixture exited unexpectedly" }
            $fixturePids += [uint32]$process.Id
        }
        $wait = [Diagnostics.Stopwatch]::StartNew()
        do {
            $observedPairs = @($fixturePids | Where-Object {
                $fixturePid = $_
                @(@($trace.StartId, $trace.StopId) | Where-Object {
                    $sourceId = $_
                    @(Get-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue |
                        Where-Object {
                            [uint32]$_.SourceEventArgs.NewEvent.ProcessID -eq $fixturePid
                        }).Count -ge 1
                }).Count -eq 2
            })
            if ($observedPairs.Count -eq $fixturePids.Count) { break }
            Start-Sleep -Milliseconds 100
        } while ($wait.ElapsedMilliseconds -lt 10000)
        if ($observedPairs.Count -ne $fixturePids.Count) {
            throw "real process trace did not receive a start and stop for every fixture"
        }
        $shutdown = [pscustomobject]@{ Pid = [uint32]0 }
        Stop-WindowsProcessTrace $trace {
            $shutdownProcess = Start-Process -FilePath $ui `
                -ArgumentList "/d", "/c", "exit 29" -Wait -PassThru
            if ($shutdownProcess.ExitCode -ne 29) {
                throw "shutdown activity fixture exited unexpectedly"
            }
            $shutdown.Pid = [uint32]$shutdownProcess.Id
        }
        $completedTrace = $trace
        $trace = $null

        $lines = @(Get-Content -LiteralPath $completedTrace.Path | ForEach-Object { $_ | ConvertFrom-Json })
        $events = @($lines | Where-Object { $_.kind -in @("start", "stop") })
        $truncated = @($lines | Where-Object { $_.kind -eq "trace-truncated" })
        if ($events.Count -ne 3 -or $truncated.Count -ne 1 -or $truncated[0].limit -ne 3) {
            throw "real process trace did not enforce its record bound"
        }
        $times = @($events | ForEach-Object { [uint64]$_.time_100ns })
        $ordered = @($times | Sort-Object)
        if (($times -join ",") -ne ($ordered -join ",")) {
            throw "real process trace records were not ordered"
        }
        if (@($events | Where-Object { $_.kind -eq "start" }).Count -eq 0 -or
            @($events | Where-Object { $_.kind -eq "stop" }).Count -eq 0 -or
            @($events | Where-Object { $_.exit_code -eq 23 }).Count -eq 0) {
            throw "real process trace did not retrieve start and stop events"
        }
        foreach ($name in $script:WindowsProcessNames) {
            if (@($events | Where-Object { $_.executable -eq $name }).Count -eq 0) {
                throw "real process trace lost an executable identity"
            }
        }
        foreach ($sourceId in @($completedTrace.StartId, $completedTrace.StopId)) {
            if (@(Get-EventSubscriber -SourceIdentifier $sourceId -ErrorAction SilentlyContinue).Count -ne 0 -or
                @(Get-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue).Count -ne 0) {
                throw "real process trace did not unregister and drain its subscription"
            }
        }
        if (@($events | Where-Object { $_.pid -eq $shutdown.Pid }).Count -ne 0) {
            throw "real process trace queued activity after shutdown began"
        }
    } finally {
        if ($null -ne $trace) {
            try { Stop-WindowsProcessTrace $trace } catch {}
        }
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
}
