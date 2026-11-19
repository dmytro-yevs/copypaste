#ifndef RUNNER_SCREENSHOT_PROTECTION_H_
#define RUNNER_SCREENSHOT_PROTECTION_H_

#include <windows.h>

// One device policy owns capture affinity for every application window.
namespace ScreenshotProtection {
// True only while blocking is enabled and applied to every registered window.
bool Blocked();
bool SetBlocked(bool blocked);
// Tracks the window even when its protection is refused; returns whether it applied.
bool Register(HWND window);
void Unregister(HWND window);
bool Apply(HWND window);
}  // namespace ScreenshotProtection

#endif  // RUNNER_SCREENSHOT_PROTECTION_H_
