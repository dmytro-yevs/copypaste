function Get-InstalledPayloadManifest([string]$InstallDirectory) {
    $root = Get-Item -LiteralPath $InstallDirectory -Force -ErrorAction Stop
    if (-not $root.PSIsContainer) { throw "installed payload root is not a directory" }
    if (($root.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "installed payload root is a reparse point"
    }
    $rootPath = $root.FullName
    $entries = @()
    foreach ($entry in @(Get-ChildItem -LiteralPath $rootPath -Force -Recurse -ErrorAction Stop)) {
        if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "installed payload contains a reparse point"
        }
        $relative = [IO.Path]::GetRelativePath($rootPath, $entry.FullName).Replace('\', '/')
        if ($relative -eq '.' -or $relative.StartsWith('../') -or [IO.Path]::IsPathRooted($relative)) {
            throw "installed payload has an invalid relative path"
        }
        if ($entry.PSIsContainer) {
            $entries += [ordered]@{ path = $relative; kind = "directory"; sha256 = $null; bytes = $null }
        } elseif ($entry.PSIsContainer -eq $false) {
            $entries += [ordered]@{
                path = $relative
                kind = "file"
                sha256 = (Get-FileHash -LiteralPath $entry.FullName -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
                bytes = [int64]$entry.Length
            }
        } else {
            throw "installed payload entry type is unreadable"
        }
    }
    return @($entries | Sort-Object path, kind)
}

function Assert-InstalledPayloadUnchanged([object[]]$Before, [object[]]$After) {
    $beforeByPath = @{}
    $afterByPath = @{}
    foreach ($entry in $Before) {
        if ($beforeByPath.ContainsKey($entry.path)) { throw "installed payload snapshot has duplicate entries" }
        $beforeByPath[$entry.path] = $entry
    }
    foreach ($entry in $After) {
        if ($afterByPath.ContainsKey($entry.path)) { throw "installed payload snapshot has duplicate entries" }
        $afterByPath[$entry.path] = $entry
    }
    foreach ($path in $beforeByPath.Keys) {
        if (-not $afterByPath.ContainsKey($path)) { throw "installer refusal removed payload $path" }
        $beforeEntry = $beforeByPath[$path]
        $afterEntry = $afterByPath[$path]
        if ($beforeEntry.kind -ne $afterEntry.kind -or
            $beforeEntry.sha256 -ne $afterEntry.sha256 -or
            $beforeEntry.bytes -ne $afterEntry.bytes) {
            throw "installer refusal changed payload $path"
        }
    }
    foreach ($path in $afterByPath.Keys) {
        if (-not $beforeByPath.ContainsKey($path)) { throw "installer refusal added payload $path" }
    }
}

function Assert-InstalledUpdateDrain([int]$InstallerExitCode, [bool]$DaemonExited) {
    if ($InstallerExitCode -ne 0) {
        throw "in-place installer update exited $InstallerExitCode"
    }
    if (-not $DaemonExited) {
        throw "in-place installer update returned before the explicit daemon exited"
    }
}

function Assert-InstalledAppLockRefusal([int]$InstallerExitCode, [bool]$AppExited, [string]$SidecarState) {
    if ($InstallerExitCode -eq 0) {
        throw "installer updated while the installed app was running"
    }
    if ($AppExited) {
        throw "installer refusal stopped the installed app"
    }
    if ($SidecarState -ne "ready") {
        throw "installer refusal changed the installed sidecar state to $SidecarState"
    }
}

function Test-WindowsInstalledScenarioHelpers {
    Assert-InstalledUpdateDrain 0 $true
    Assert-InstalledAppLockRefusal 1 $false "ready"
    $root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-installed-payload-$([guid]::NewGuid())"
    try {
        [IO.Directory]::CreateDirectory((Join-Path $root "nested")) | Out-Null
        [IO.File]::WriteAllText((Join-Path $root "payload.txt"), "before")
        [IO.File]::WriteAllText((Join-Path $root "nested/child.txt"), "child")
        $baseline = Get-InstalledPayloadManifest $root
        Assert-InstalledPayloadUnchanged $baseline (Get-InstalledPayloadManifest $root)
        foreach ($mutation in @(
            { [IO.File]::WriteAllText((Join-Path $root "payload.txt"), "changed") },
            { [IO.File]::WriteAllText((Join-Path $root "added.txt"), "added") },
            { Remove-Item -LiteralPath (Join-Path $root "nested/child.txt") -Force }
        )) {
            & $mutation
            $rejected = $false
            try { Assert-InstalledPayloadUnchanged $baseline (Get-InstalledPayloadManifest $root) } catch { $rejected = $true }
            if (-not $rejected) { throw "installed payload change passed the refusal proof" }
            Remove-Item -LiteralPath $root -Recurse -Force
            [IO.Directory]::CreateDirectory((Join-Path $root "nested")) | Out-Null
            [IO.File]::WriteAllText((Join-Path $root "payload.txt"), "before")
            [IO.File]::WriteAllText((Join-Path $root "nested/child.txt"), "child")
        }
        $rejected = $false
        try { Get-InstalledPayloadManifest (Join-Path $root "missing") | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw "missing installed payload root passed the refusal proof" }
        $link = Join-Path $root "payload-link"
        try {
            New-Item -ItemType SymbolicLink -Path $link -Target (Join-Path $root "payload.txt") -ErrorAction Stop | Out-Null
            $rejected = $false
            try { Get-InstalledPayloadManifest $root | Out-Null } catch { $rejected = $true }
            if (-not $rejected) { throw "reparse point passed the installed payload proof" }
        } catch [System.PlatformNotSupportedException] {
            # The manifest still rejects reparse points; this host cannot create one for the fixture.
        } finally {
            Remove-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
        }
    } finally {
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
    foreach ($fixture in @(
        { Assert-InstalledUpdateDrain 1 $true },
        { Assert-InstalledUpdateDrain 0 $false },
        { Assert-InstalledAppLockRefusal 0 $false "ready" },
        { Assert-InstalledAppLockRefusal 1 $true "ready" },
        { Assert-InstalledAppLockRefusal 1 $false "not-ready" }
    )) {
        $rejected = $false
        try { & $fixture } catch { $rejected = $true }
        if (-not $rejected) { throw "installed scenario helper accepted an unsafe fixture" }
    }
}
