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
