Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class CopyPasteNativeWindowEvidence {
    [StructLayout(LayoutKind.Sequential)] private struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] private struct Point { public int X, Y; }
    [DllImport("user32.dll", EntryPoint="ClientToScreen")] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool NativeClientToScreen(IntPtr hWnd, ref Point point);
    [DllImport("user32.dll", EntryPoint="GetClientRect")] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool NativeGetClientRect(IntPtr hWnd, out Rect rect);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool GetWindowDisplayAffinity(IntPtr hWnd, out uint affinity);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool ShowWindowAsync(IntPtr hWnd, int command);
    public static bool TryGetClientBounds(IntPtr hWnd, out int x, out int y, out int width, out int height) {
        x = y = width = height = 0;
        Rect rect;
        if (!NativeGetClientRect(hWnd, out rect)) return false;
        Point topLeft = new Point { X = rect.Left, Y = rect.Top };
        Point bottomRight = new Point { X = rect.Right, Y = rect.Bottom };
        if (!NativeClientToScreen(hWnd, ref topLeft) || !NativeClientToScreen(hWnd, ref bottomRight)) return false;
        x = topLeft.X; y = topLeft.Y; width = bottomRight.X - topLeft.X; height = bottomRight.Y - topLeft.Y;
        return width > 0 && height > 0;
    }
}
"@

function Get-WindowCaptureState([IntPtr]$Handle) {
    [uint32]$affinity = 0
    $affinityRead = [CopyPasteNativeWindowEvidence]::GetWindowDisplayAffinity($Handle, [ref]$affinity)
    return [ordered]@{
        handle = $Handle.ToInt64()
        foreground = [CopyPasteNativeWindowEvidence]::GetForegroundWindow() -eq $Handle
        visible = [CopyPasteNativeWindowEvidence]::IsWindowVisible($Handle)
        minimized = [CopyPasteNativeWindowEvidence]::IsIconic($Handle)
        capture_allowed = $affinityRead -and $affinity -eq 0
        display_affinity = if ($affinityRead) { [int64]$affinity } else { -1 }
    }
}

function Test-WindowCaptureReady($State) {
    return $State.foreground -and $State.visible -and -not $State.minimized -and $State.capture_allowed
}

function Get-WindowActivationPlan($State) {
    return [ordered]@{
        restore = [bool]$State.minimized
        activate = -not [bool]$State.foreground
    }
}

function New-WindowCaptureObservation([string]$Phase, $State) {
    return [ordered]@{
        phase = $Phase
        handle = [int64]$State.handle
        foreground = [bool]$State.foreground
        visible = [bool]$State.visible
        minimized = [bool]$State.minimized
        capture_allowed = [bool]$State.capture_allowed
        display_affinity = [int64]$State.display_affinity
    }
}

function Write-WindowCaptureObservation(
    [Diagnostics.Process]$App,
    [string]$Path,
    [string]$Phase,
    $State = $null
) {
    if (-not $Path) { return }
    $App.Refresh()
    if ($null -eq $State) {
        $State = Get-WindowCaptureState $App.MainWindowHandle
    }
    $record = New-WindowCaptureObservation $Phase $State
    $record["utc"] = [DateTime]::UtcNow.ToString("o")
    $record | ConvertTo-Json -Compress | Add-Content -LiteralPath $Path -Encoding utf8
}

function Get-ClientCaptureBounds([IntPtr]$Handle) {
    [int]$x = 0
    [int]$y = 0
    [int]$width = 0
    [int]$height = 0
    $held = [CopyPasteNativeWindowEvidence]::TryGetClientBounds(
        $Handle, [ref]$x, [ref]$y, [ref]$width, [ref]$height
    )
    Assert-True $held "could not derive the exact CopyPaste client bounds from HWND $($Handle.ToInt64())"
    Assert-True ($width -le 16384 -and $height -le 16384) "CopyPaste client bounds exceed the capture limit: ${width}x$height"
    return [ordered]@{ kind = "client"; x = $x; y = $y; width = $width; height = $height }
}

function Wait-ForegroundWindow([Diagnostics.Process]$App) {
    return Wait-Readiness "exact CopyPaste window foreground readiness" {
        $App.Refresh()
        if ($App.HasExited) { return New-ProbeInvariant "the app exited with code $($App.ExitCode)" }
        $handle = $App.MainWindowHandle
        if ($handle -eq [IntPtr]::Zero) { return New-ProbeNotReady "the app has published no native window handle" }
        $state = Get-WindowCaptureState $handle
        $plan = Get-WindowActivationPlan $state
        if ($plan.restore) {
            # Run 33124469581 lost WDA_NONE while probing a settled HWND.
            [CopyPasteNativeWindowEvidence]::ShowWindowAsync($handle, 9) | Out-Null
        }
        if ($plan.activate) {
            [CopyPasteNativeWindowEvidence]::SetForegroundWindow($handle) | Out-Null
        }
        if ($plan.restore -or $plan.activate) {
            $state = Get-WindowCaptureState $handle
        }
        if (Test-WindowCaptureReady $state) { return New-ProbeReady $state }
        return New-ProbeNotReady "foreground=$($state.foreground) visible=$($state.visible) minimized=$($state.minimized) display affinity=$($state.display_affinity)"
    } {
        $App.Refresh()
        $handle = $App.MainWindowHandle
        $state = Get-WindowCaptureState $handle
        "expected HWND $($handle.ToInt64()); foreground HWND $([CopyPasteNativeWindowEvidence]::GetForegroundWindow().ToInt64()); visible=$($state.visible); minimized=$($state.minimized); display affinity=$($state.display_affinity)"
    }
}

function Save-WindowImage(
    [Diagnostics.Process]$App,
    [string]$Path,
    [string]$CaptureTracePath = "",
    [string]$ObservationPhase = "capture"
) {
    Write-WindowCaptureObservation $App $CaptureTracePath "$ObservationPhase/pre-activation"
    $captureState = Wait-ForegroundWindow $App
    $handle = [IntPtr]$captureState.handle
    $bounds = Get-ClientCaptureBounds $handle
    $captureState["capture_bounds"] = $bounds
    $bitmap = [Drawing.Bitmap]::new($bounds.width, $bounds.height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $immediateState = Get-WindowCaptureState $handle
        Write-WindowCaptureObservation $App $CaptureTracePath "$ObservationPhase/pre-capture" $immediateState
        Assert-True (Test-WindowCaptureReady $immediateState) "CopyPaste lost foreground immediately before capture"
        $graphics.CopyFromScreen($bounds.x, $bounds.y, 0, 0, $bitmap.Size)
        $bitmap.Save($Path, [Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
    return $captureState
}
