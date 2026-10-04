#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_

#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>

#include <memory>
#include <cstdint>
#include <string>

#include "win32_window.h"
#include "app_update_channel.h"

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
  void ShowQuickPasteAtCursor();
  bool PasteIntoPreviousWindow();
  void ShowMainWindow(bool open_settings);

  // The project to run.
  flutter::DartProject project_;
  bool is_protected_pairing_context_;
  std::string pairing_context_id_;
  std::string pairing_ceremony_id_;
  std::string pending_pairing_uri_;
  bool is_quick_paste_context_;
  HWND main_window_handle_ = nullptr;
  HWND previous_foreground_window_ = nullptr;
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
