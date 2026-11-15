#ifndef RUNNER_WINDOW_MATERIAL_H_
#define RUNNER_WINDOW_MATERIAL_H_

#include <flutter/binary_messenger.h>
#include <flutter/encodable_value.h>
#include <flutter/method_channel.h>
#include <windows.h>

#include <memory>
#include <optional>

// Shows the Windows 11 system backdrop behind one Flutter window while the
// glass preference is on and the system keeps transparency effects. Windows
// 10 rejects the backdrop attribute, so the window stays opaque there.
class WindowMaterial {
 public:
  enum class Role { kMain, kTransient };

  WindowMaterial(flutter::BinaryMessenger* messenger, HWND window, Role role);
  ~WindowMaterial();

  WindowMaterial(const WindowMaterial&) = delete;
  WindowMaterial& operator=(const WindowMaterial&) = delete;

  // Re-reads the transparency, contrast and accent settings after a system
  // change and reports what changed to Dart.
  void HandleSettingChange();

 private:
  flutter::EncodableValue Apply();

  HWND window_;
  Role role_;
  bool supported_ = false;
  bool enabled_ = false;
  bool active_ = false;
  bool reduced_ = false;
  std::optional<int64_t> accent_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      accent_channel_;
};

#endif  // RUNNER_WINDOW_MATERIAL_H_
