#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_

#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>

#include <functional>
#include <memory>
#include <optional>
#include <cstdint>
#include <string>

#include "win32_window.h"
#include "app_update_channel.h"

// OS-call seam for the existing Quick Paste presentation operation.
struct QuickPastePresentationApi {
  decltype(&::GetCursorPos) get_cursor_pos = &::GetCursorPos;
  decltype(&::MonitorFromPoint) monitor_from_point = &::MonitorFromPoint;
  decltype(&::GetMonitorInfoW) get_monitor_info = &::GetMonitorInfoW;
  decltype(&::SetWindowPos) set_window_pos = &::SetWindowPos;
  decltype(&::GetDpiForWindow) get_dpi_for_window = &::GetDpiForWindow;
  decltype(&::SetForegroundWindow) set_foreground_window = &::SetForegroundWindow;
  decltype(&::ShowWindow) show_window = &::ShowWindow;
};

bool PresentQuickPasteAtCursor(
    HWND window, const QuickPastePresentationApi& api,
    const std::function<void()>& opened,
    const std::function<bool()>& owns = [] { return true; },
    const std::function<void()>& cleanup = {});

// Public Win32 calls used by the presentation-scoped target decision flow.
struct QuickPasteTargetApi {
  decltype(&::GetForegroundWindow) get_foreground_window = &::GetForegroundWindow;
  decltype(&::GetAncestor) get_ancestor = &::GetAncestor;
  decltype(&::GetWindowThreadProcessId) get_owner = &::GetWindowThreadProcessId;
  decltype(&::GetCurrentProcessId) get_current_process_id = &::GetCurrentProcessId;
  decltype(&::IsWindow) is_window = &::IsWindow;
  decltype(&::IsWindowVisible) is_window_visible = &::IsWindowVisible;
  decltype(&::ShowWindow) show_window = &::ShowWindow;
  decltype(&::SetForegroundWindow) set_foreground_window = &::SetForegroundWindow;
  decltype(&::SendInput) send_input = &::SendInput;
};

class QuickPasteTargetSession {
 public:
  explicit QuickPasteTargetSession(QuickPasteTargetApi api = {}) : api_(api) {}
  int64_t Begin(HWND sampled_foreground, HWND popup);
  void Invalidate();
  void Close(int64_t id, HWND popup);
  bool Paste(int64_t id, HWND popup);
  int64_t id() const { return id_; }
  bool internally_hiding() const { return internally_hiding_; }

 private:
  struct Target { HWND root; DWORD pid; DWORD tid; };
  std::optional<Target> Capture(HWND window) const;
  bool Valid(const Target& target) const;
  void Hide(HWND popup);
  QuickPasteTargetApi api_;
  std::optional<Target> target_;
  int64_t counter_ = 0;
  int64_t id_ = 0;
  bool pasting_ = false;
  bool internally_hiding_ = false;
};

bool ShowQuickPastePresentationAtCursor(
    HWND window, QuickPasteTargetSession& session, int64_t id,
    const QuickPastePresentationApi& api, const std::function<void()>& opened);

// A window that does nothing but host a Flutter view.
class FlutterWindow : public Win32Window {
 public:
  // Creates a new FlutterWindow hosting a Flutter view running |project|.
  explicit FlutterWindow(const flutter::DartProject& project,
                         bool is_protected_pairing_context = false,
                         std::string pairing_context_id = "",
                         std::string pairing_ceremony_id = "",
                         std::string pending_pairing_uri = "",
                         bool is_quick_paste_context = false);
  virtual ~FlutterWindow();

 protected:
  // Win32Window:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT const message, WPARAM const wparam,
                         LPARAM const lparam) noexcept override;

 private:
  bool SetCaptureProtection(bool enabled);
  bool BeginProtectedPairingContext();
  bool IsProtectedPairingContextActive() const;
  bool DetachProtectedPairingContext();
  bool CancelProtectedPairingContext();
  std::string OpenProtectedPairingContext(const std::string& ceremony_id);
  void CloseProtectedPairingContext(const std::string& context_id);
  bool PrepareQuickPasteContext();
  bool OpenQuickPasteContext();
  void CloseQuickPasteContext();
  bool ShowQuickPasteAtCursor(HWND sampled_foreground);
  bool PasteIntoPreviousWindow(int64_t presentation_id);
  void ShowMainWindow(bool open_settings);

  // The project to run.
  flutter::DartProject project_;
  bool is_protected_pairing_context_;
  std::string pairing_context_id_;
  std::string pairing_ceremony_id_;
  std::string pending_pairing_uri_;
  bool is_quick_paste_context_;
  HWND main_window_handle_ = nullptr;
  QuickPasteTargetSession quick_paste_session_;
  uint64_t pairing_generation_ = 0;
  bool pairing_cancellation_approved_ = false;

  // The Flutter instance hosted by this window.
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      pairing_presentation_channel_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      pairing_links_channel_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      quick_paste_channel_;
  std::unique_ptr<AppUpdateChannel> app_update_channel_;
  std::unique_ptr<FlutterWindow> protected_pairing_window_;
  std::unique_ptr<FlutterWindow> quick_paste_window_;
};

#endif  // RUNNER_FLUTTER_WINDOW_H_
