#ifndef RUNNER_SCREENSHOT_PROTECTION_H_
#define RUNNER_SCREENSHOT_PROTECTION_H_

#include <windows.h>

// One device policy owns capture affinity for every application window.
namespace ScreenshotProtection {
bool Blocked();
bool SetBlocked(bool blocked);
bool Register(HWND window);
void Unregister(HWND window);
bool Apply(HWND window);
}  // namespace ScreenshotProtection

#endif  // RUNNER_SCREENSHOT_PROTECTION_H_
