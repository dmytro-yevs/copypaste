$script:WindowsProcessTraceLimit = 128
$script:WindowsProcessNames = @("copypaste-ui.exe", "copypaste-daemon.exe")

function ConvertTo-WindowsProcessTraceLine([string]$Kind, $Record, $StartedNames = @{}) {
    $name = [IO.Path]::GetFileName(([string]$Record.ProcessName)).ToLowerInvariant()
    $processId = [uint32]$Record.ProcessID
    if ($Kind -eq "stop" -and $name -notin $script:WindowsProcessNames -and
        $StartedNames.ContainsKey($processId)) {
        $name = $StartedNames[$processId]
    }
    if ($name -notin $script:WindowsProcessNames) { return $null }
    $line = [ordered]@{
        time_100ns = [uint64]$Record.TIME_CREATED
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
        Unregister-Event -SourceIdentifier $startId -ErrorAction SilentlyContinue
        Unregister-Event -SourceIdentifier $stopId -ErrorAction SilentlyContinue
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

function Stop-WindowsProcessTrace($Trace) {
    if ($null -eq $Trace -or -not $Trace.Available) { return }
    try {
        Start-Sleep -Milliseconds 300
        $records = @()
        foreach ($source in @(
            @{ Id = $Trace.StartId; Kind = "start" },
            @{ Id = $Trace.StopId; Kind = "stop" }
        )) {
            foreach ($event in @(Get-Event -SourceIdentifier $source.Id -ErrorAction SilentlyContinue)) {
                $records += [pscustomobject]@{
                    Kind = $source.Kind
                    Record = $event.SourceEventArgs.NewEvent
                }
            }
        }
        $startedNames = @{}
        foreach ($record in @($records | Where-Object { $_.Kind -eq "start" })) {
            $name = [IO.Path]::GetFileName(([string]$record.Record.ProcessName)).ToLowerInvariant()
            if ($name -in $script:WindowsProcessNames) {
                $startedNames[[uint32]$record.Record.ProcessID] = $name
            }
        }
        $converted = @(
            $records |
                Sort-Object { [uint64]$_.Record.TIME_CREATED }, Kind |
                ForEach-Object { ConvertTo-WindowsProcessTraceLine $_.Kind $_.Record $startedNames } |
                Where-Object { $null -ne $_ }
        )
        $lines = @($converted | Select-Object -First $Trace.Limit)
        if ($converted.Count -gt $Trace.Limit) {
            $lines += ([ordered]@{ kind = "trace-truncated"; limit = $Trace.Limit } |
                ConvertTo-Json -Compress)
        }
        if ($lines.Count -eq 0) {
            $lines = @('{"kind":"trace-empty"}')
        }
        $lines | Set-Content -LiteralPath $Trace.Path -Encoding utf8
    } finally {
        foreach ($sourceId in @($Trace.StartId, $Trace.StopId)) {
            Get-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue |
                Remove-Event -ErrorAction SilentlyContinue
            Unregister-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue
        }
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

    $startedNames = @{}
    $startedNames[[uint32]42] = "copypaste-daemon.exe"
    $stop = ConvertTo-WindowsProcessTraceLine "stop" ([pscustomobject]@{
        TIME_CREATED = 200
        ProcessName = "copypaste-daem"
        ProcessID = 42
        ParentProcessID = 41
        ExitStatus = [uint32]3221225477
    }) $startedNames | ConvertFrom-Json
    if ($stop.kind -ne "stop" -or $stop.executable -ne "copypaste-daemon.exe" -or
        $stop.exit_code -ne 3221225477) {
        throw "process trace stop records lost the exit code"
    }

    $ignored = ConvertTo-WindowsProcessTraceLine "start" ([pscustomobject]@{
        TIME_CREATED = 300
        ProcessName = "private-secret.exe"
        ProcessID = 43
        ParentProcessID = 7
    })
    if ($null -ne $ignored) {
        throw "process trace accepted an unrelated executable identity"
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
        foreach ($executable in @($ui, $daemon)) {
            $process = Start-Process -FilePath $executable `
                -ArgumentList "/d", "/c", "exit 23" -Wait -PassThru
            if ($process.ExitCode -ne 23) { throw "process trace fixture exited unexpectedly" }
            Start-Sleep -Milliseconds 250
        }
        $wait = [Diagnostics.Stopwatch]::StartNew()
        do {
            $observed = @(
                @($trace.StartId, $trace.StopId) |
                    ForEach-Object { Get-Event -SourceIdentifier $_ -ErrorAction SilentlyContinue }
            ).Count
            if ($observed -ge 4) { break }
            Start-Sleep -Milliseconds 100
        } while ($wait.ElapsedMilliseconds -lt 5000)
        if ($observed -lt 4) { throw "real process trace did not receive every fixture event" }
        Stop-WindowsProcessTrace $trace
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
    } finally {
        if ($null -ne $trace) {
            try { Stop-WindowsProcessTrace $trace } catch {}
        }
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
}
