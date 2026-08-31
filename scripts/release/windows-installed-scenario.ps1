function Get-InstalledPayloadRelativePath([string]$RootPath, [string]$EntryPath) {
    $separator = [IO.Path]::DirectorySeparatorChar
    $basePath = $RootPath.TrimEnd(@([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)) + $separator
    $baseUri = [Uri]::new($basePath, [UriKind]::Absolute)
    $entryUri = [Uri]::new($EntryPath, [UriKind]::Absolute)
    $relativeUri = $baseUri.MakeRelativeUri($entryUri)
    if ($relativeUri.IsAbsoluteUri) { throw "installed payload has an invalid relative path" }
    $relative = [Uri]::UnescapeDataString($relativeUri.ToString()).Replace('\', '/')
    if ($relative -eq '.' -or $relative.StartsWith('../') -or
        $relative -match '^(?:[A-Za-z]:/|//)' -or [IO.Path]::IsPathRooted($relative)) {
        throw "installed payload has an invalid relative path"
    }
    return $relative
}

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
        $relative = Get-InstalledPayloadRelativePath $rootPath $entry.FullName
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
    if ((Get-InstalledPayloadRelativePath 'C:\payload' 'C:\payload\nested\file # %.txt') -ne 'nested/file # %.txt') {
        throw "Windows payload relative path compatibility contract failed"
    }
    $rejected = $false
    try { Get-InstalledPayloadRelativePath 'C:\payload' 'C:\other\file.txt' | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw "Windows outside payload path passed the refusal proof" }
    foreach ($path in @('D:\payload\file.txt', '\\other-server\payload\file.txt')) {
        $rejected = $false
        try { Get-InstalledPayloadRelativePath 'C:\payload' $path | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw "different Windows payload authority passed the refusal proof" }
    }
    $root = Join-Path ([IO.Path]::GetTempPath()) "copypaste-installed-payload-$([guid]::NewGuid())"
    try {
        $restoreFixture = {
            Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
            [IO.Directory]::CreateDirectory((Join-Path $root "nested")) | Out-Null
            [IO.File]::WriteAllText((Join-Path $root "payload.txt"), "before")
            [IO.File]::WriteAllText((Join-Path $root "nested/child.txt"), "child")
            [IO.File]::WriteAllText((Join-Path $root "nested/name # %.txt"), "escaped")
        }
        & $restoreFixture
        if ((Get-InstalledPayloadRelativePath $root (Join-Path $root "nested/name # %.txt")) -ne "nested/name # %.txt") {
            throw "installed payload relative path did not preserve escaped filename characters"
        }
        $outside = Join-Path ([IO.Path]::GetDirectoryName($root)) "copypaste-installed-payload-outside-$([guid]::NewGuid()).txt"
        [IO.File]::WriteAllText($outside, "outside")
        $rejected = $false
        try { Get-InstalledPayloadRelativePath $root $outside | Out-Null } catch { $rejected = $true }
        Remove-Item -LiteralPath $outside -Force
        if (-not $rejected) { throw "outside installed payload path passed the refusal proof" }
        $baseline = Get-InstalledPayloadManifest $root
        Assert-InstalledPayloadUnchanged $baseline (Get-InstalledPayloadManifest $root)
        foreach ($snapshots in @(
            @{ before = @($baseline[0], $baseline[0]); after = $baseline },
            @{ before = $baseline; after = @($baseline[0], $baseline[0]) }
        )) {
            $rejected = $false
            try { Assert-InstalledPayloadUnchanged $snapshots.before $snapshots.after } catch { $rejected = $true }
            if (-not $rejected) { throw "duplicate installed payload snapshot passed the refusal proof" }
        }
        foreach ($mutation in @(
            { [IO.File]::WriteAllText((Join-Path $root "payload.txt"), "changed") },
            { [IO.File]::WriteAllText((Join-Path $root "added.txt"), "added") },
            { [IO.Directory]::CreateDirectory((Join-Path $root "added-directory")) | Out-Null },
            { Remove-Item -LiteralPath (Join-Path $root "nested") -Recurse -Force }
        )) {
            Assert-InstalledPayloadUnchanged $baseline (Get-InstalledPayloadManifest $root)
            & $mutation
            $rejected = $false
            try { Assert-InstalledPayloadUnchanged $baseline (Get-InstalledPayloadManifest $root) } catch { $rejected = $true }
            if (-not $rejected) { throw "installed payload change passed the refusal proof" }
            & $restoreFixture
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
        $rootLink = Join-Path ([IO.Path]::GetDirectoryName($root)) "copypaste-installed-payload-link-$([guid]::NewGuid())"
        try {
            New-Item -ItemType SymbolicLink -Path $rootLink -Target $root -ErrorAction Stop | Out-Null
            $rejected = $false
            try { Get-InstalledPayloadManifest $rootLink | Out-Null } catch { $rejected = $true }
            if (-not $rejected) { throw "reparse payload root passed the refusal proof" }
        } catch [System.PlatformNotSupportedException] {
            # The manifest still rejects reparse roots; this host cannot create one for the fixture.
        } finally {
            Remove-Item -LiteralPath $rootLink -Force -ErrorAction SilentlyContinue
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
