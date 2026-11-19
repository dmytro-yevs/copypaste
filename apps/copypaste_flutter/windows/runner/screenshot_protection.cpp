#include "screenshot_protection.h"

#include <set>

namespace {
constexpr wchar_t kRegistryPath[] = L"Software\\CopyPaste\\Security";
constexpr wchar_t kRegistryValue[] = L"BlockScreenshots";

bool ReadBlocked() {
  DWORD value = 0;
  DWORD size = sizeof(value);
  return RegGetValueW(HKEY_CURRENT_USER, kRegistryPath, kRegistryValue,
                     RRF_RT_REG_DWORD, nullptr, &value, &size) == ERROR_SUCCESS &&
         value != 0;
}

bool blocked = ReadBlocked();
std::set<HWND> windows;
// Windows whose capture affinity was refused, e.g. under Remote Desktop.
std::set<HWND> unprotected;

bool ApplyValue(HWND window, bool value) {
  return SetWindowDisplayAffinity(window, value ? WDA_EXCLUDEFROMCAPTURE : WDA_NONE) != FALSE;
}
}  // namespace

namespace ScreenshotProtection {
bool Blocked() { return blocked && unprotected.empty(); }

bool Apply(HWND window) { return !blocked || ApplyValue(window, true); }

bool Register(HWND window) {
  windows.insert(window);
  if (Apply(window)) return true;
  unprotected.insert(window);
  return false;
}

void Unregister(HWND window) {
  windows.erase(window);
  unprotected.erase(window);
}

bool SetBlocked(bool value) {
  if (value == blocked && (!value || unprotected.empty())) return true;
  for (HWND window : windows) {
    if (!ApplyValue(window, value)) {
      for (HWND restore : windows) ApplyValue(restore, blocked);
      return false;
    }
  }
  HKEY key = nullptr;
  if (RegCreateKeyExW(HKEY_CURRENT_USER, kRegistryPath, 0, nullptr, 0,
                     KEY_SET_VALUE, nullptr, &key, nullptr) != ERROR_SUCCESS) {
    for (HWND window : windows) ApplyValue(window, blocked);
    return false;
  }
  const DWORD stored = value ? 1 : 0;
  const auto status = RegSetValueExW(key, kRegistryValue, 0, REG_DWORD,
      reinterpret_cast<const BYTE*>(&stored), sizeof(stored));
  RegCloseKey(key);
  if (status != ERROR_SUCCESS) {
    for (HWND window : windows) ApplyValue(window, blocked);
    return false;
  }
  blocked = value;
  unprotected.clear();
  return true;
}
}  // namespace ScreenshotProtection
