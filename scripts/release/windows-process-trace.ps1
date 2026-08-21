$script:WindowsProcessTraceLimit = 128
$script:WindowsProcessNames = @("copypaste-ui.exe", "copypaste-daemon.exe")

function ConvertTo-WindowsProcessTraceLine([string]$Kind, $Record) {
    $name = [IO.Path]::GetFileName(([string]$Record.ProcessName)).ToLowerInvariant()
    if ($name -notin $script:WindowsProcessNames) { return $null }
    $line = [ordered]@{
        time_100ns = [uint64]$Record.TIME_CREATED
        kind = $Kind
        executable = $name
        pid = [uint32]$Record.ProcessID
        parent_pid = [uint32]$Record.ParentProcessID
    }
    if ($Kind -eq "stop") {
        $line.exit_code = [uint32]$Record.ExitStatus
    }
    return $line | ConvertTo-Json -Compress
}

function Start-WindowsProcessTrace([string]$Path) {
    $token = [guid]::NewGuid().ToString("N")
    $startId = "CopyPasteProcessStart-$token"
    $stopId = "CopyPasteProcessStop-$token"
    $filter = "ProcessName = 'copypaste-ui.exe' OR ProcessName = 'copypaste-daemon.exe'"
    try {
        Register-CimIndicationEvent -Namespace root/cimv2 `
            -Query "SELECT * FROM Win32_ProcessStartTrace WHERE $filter" `
            -SourceIdentifier $startId | Out-Null
        Register-CimIndicationEvent -Namespace root/cimv2 `
            -Query "SELECT * FROM Win32_ProcessStopTrace WHERE $filter" `
            -SourceIdentifier $stopId | Out-Null
        return [pscustomobject]@{
            Path = $Path
            StartId = $startId
            StopId = $stopId
            Available = $true
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
        }
    }
}

function Stop-WindowsProcessTrace($Trace) {
    if ($null -eq $Trace -or -not $Trace.Available) { return }
    Start-Sleep -Milliseconds 300
    $records = @()
    foreach ($source in @(
        @{ Id = $Trace.StartId; Kind = "start" },
        @{ Id = $Trace.StopId; Kind = "stop" }
    )) {
        foreach ($event in @(Get-Event -SourceIdentifier $source.Id -ErrorAction SilentlyContinue)) {
            $records += [pscustomobject]@{
                Kind = $source.Kind
                Event = $event
                Record = $event.SourceEventArgs.NewEvent
            }
        }
    }
    $lines = @(
        $records |
            Sort-Object { [uint64]$_.Record.TIME_CREATED }, Kind |
            Select-Object -First $script:WindowsProcessTraceLimit |
            ForEach-Object { ConvertTo-WindowsProcessTraceLine $_.Kind $_.Record } |
            Where-Object { $null -ne $_ }
    )
    if ($records.Count -gt $script:WindowsProcessTraceLimit) {
        $lines += '{"kind":"trace-truncated","limit":128}'
    }
    if ($lines.Count -eq 0) {
        $lines = @('{"kind":"trace-empty"}')
    }
    $lines | Set-Content -LiteralPath $Trace.Path -Encoding utf8
    foreach ($sourceId in @($Trace.StartId, $Trace.StopId)) {
        Get-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue |
            Remove-Event -ErrorAction SilentlyContinue
        Unregister-Event -SourceIdentifier $sourceId -ErrorAction SilentlyContinue
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

    $stop = ConvertTo-WindowsProcessTraceLine "stop" ([pscustomobject]@{
        TIME_CREATED = 200
        ProcessName = "copypaste-daemon.exe"
        ProcessID = 42
        ParentProcessID = 41
        ExitStatus = 23
    }) | ConvertFrom-Json
    if ($stop.kind -ne "stop" -or $stop.exit_code -ne 23) {
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
