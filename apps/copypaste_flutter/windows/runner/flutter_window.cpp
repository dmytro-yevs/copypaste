#include "flutter_window.h"

#include <algorithm>
#include <optional>
#include <limits>
#include <string>
#include <utility>
#include <vector>

#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>

#include "flutter/generated_plugin_registrant.h"
#include "copypaste_flutter_protected_pairing.h"

namespace {

constexpr size_t kMaximumArtifactBytes = 16 * 1024 * 1024;
constexpr char kQuickPasteHostChannel[] =
    "com.copypaste.app/quick_paste_host";
constexpr char kQuickPasteContextChannel[] =
    "com.copypaste.app/quick_paste_context";

template <typename Function>
Function ResolveProtectedPairingSymbol(const char* symbol) {
  const HMODULE bridge = GetModuleHandleW(L"copypaste_flutter_bridge.dll");
  return bridge == nullptr ? nullptr
      : reinterpret_cast<Function>(GetProcAddress(bridge, symbol));
}

std::optional<std::vector<uint8_t>> CopyAndReleaseBuffer(
    copypaste_flutter_protected_pairing_buffer buffer) {
  const auto free_buffer = ResolveProtectedPairingSymbol<
      decltype(&copypaste_flutter_free_protected_pairing_buffer)>(
          "copypaste_flutter_free_protected_pairing_buffer");
  if (buffer.bytes == nullptr || buffer.len == 0 ||
      buffer.len > kMaximumArtifactBytes || free_buffer == nullptr) {
    if (buffer.bytes != nullptr && free_buffer != nullptr) free_buffer(buffer);
    return std::nullopt;
  }
  std::vector<uint8_t> copy(buffer.bytes, buffer.bytes + buffer.len);
  free_buffer(buffer);
  return copy;
}

const std::string* StringArgument(const flutter::MethodCall<flutter::EncodableValue>& call,
                                  const char* name) {
  const auto* arguments = std::get_if<flutter::EncodableMap>(call.arguments());
  if (arguments == nullptr) return nullptr;
  const auto iterator = arguments->find(flutter::EncodableValue(name));
  return iterator == arguments->end()
      ? nullptr : std::get_if<std::string>(&iterator->second);
}

const bool* BoolArgument(const flutter::MethodCall<flutter::EncodableValue>& call,
                         const char* name) {
  const auto* arguments = std::get_if<flutter::EncodableMap>(call.arguments());
  if (arguments == nullptr) return nullptr;
  const auto iterator = arguments->find(flutter::EncodableValue(name));
  return iterator == arguments->end()
      ? nullptr : std::get_if<bool>(&iterator->second);
}

}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project,
                             bool is_protected_pairing_context,
                             std::string pairing_context_id,
                             std::string pairing_ceremony_id,
                             std::string pending_pairing_uri,
                             bool is_quick_paste_context)
    : project_(project),
      is_protected_pairing_context_(is_protected_pairing_context),
      pairing_context_id_(std::move(pairing_context_id)),
      pairing_ceremony_id_(std::move(pairing_ceremony_id)),
      pending_pairing_uri_(std::move(pending_pairing_uri)),
      is_quick_paste_context_(is_quick_paste_context) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  if ((!pending_pairing_uri_.empty() && !SetCaptureProtection(true)) ||
      (is_protected_pairing_context_ &&
       (!SetCaptureProtection(true) || !BeginProtectedPairingContext()))) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  if (!is_protected_pairing_context_ && !is_quick_paste_context_) {
    app_update_channel_ = std::make_unique<AppUpdateChannel>(
        flutter_controller_->engine()->messenger(), GetHandle());
  }
  if (is_quick_paste_context_) {
    quick_paste_channel_ =
        std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
            flutter_controller_->engine()->messenger(),
            kQuickPasteContextChannel,
            &flutter::StandardMethodCodec::GetInstance());
    quick_paste_channel_->SetMethodCallHandler(
        [this](const auto& call, auto result) {
          if (call.method_name() == "accessibilityGranted" ||
              call.method_name() == "requestAccessibility") {
            result->Success(flutter::EncodableValue(true));
          } else if (call.method_name() == "paste") {
            result->Success(flutter::EncodableValue(PasteIntoPreviousWindow()));
          } else if (call.method_name() == "close") {
            ::ShowWindow(GetHandle(), SW_HIDE);
            result->Success(flutter::EncodableValue(true));
          } else if (call.method_name() == "openMain") {
            ShowMainWindow(false);
            result->Success(flutter::EncodableValue(true));
          } else if (call.method_name() == "openSettings") {
            ShowMainWindow(true);
            result->Success(flutter::EncodableValue(true));
          } else if (call.method_name() == "quit") {
            result->Success(flutter::EncodableValue(true));
            ::PostQuitMessage(0);
          } else {
            result->NotImplemented();
          }
        });
  } else if (!is_protected_pairing_context_) {
    quick_paste_channel_ =
        std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
            flutter_controller_->engine()->messenger(),
            kQuickPasteHostChannel,
            &flutter::StandardMethodCodec::GetInstance());
    quick_paste_channel_->SetMethodCallHandler(
        [this](const auto& call, auto result) {
          if (call.method_name() == "isSupported") {
            result->Success(flutter::EncodableValue(true));
          } else if (call.method_name() == "prepare") {
            result->Success(
                flutter::EncodableValue(PrepareQuickPasteContext()));
          } else if (call.method_name() == "open") {
            result->Success(flutter::EncodableValue(OpenQuickPasteContext()));
          } else if (call.method_name() == "accessibilityGranted" ||
                     call.method_name() == "requestAccessibility") {
            result->Success(flutter::EncodableValue(true));
          } else if (call.method_name() == "dispose") {
            CloseQuickPasteContext();
            result->Success(flutter::EncodableValue(true));
          } else {
            result->NotImplemented();
          }
        });
  }
  pairing_links_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(),
          "com.copypaste.app/pairing_links",
          &flutter::StandardMethodCodec::GetInstance());
  pairing_links_channel_->SetMethodCallHandler(
      [this](const auto& call, auto result) {
        if (call.method_name() != "takePendingUri") {
          result->NotImplemented();
          return;
        }
        if (pending_pairing_uri_.empty()) {
          result->Success(flutter::EncodableValue());
          return;
        }
        auto uri = std::move(pending_pairing_uri_);
        pending_pairing_uri_.clear();
        result->Success(flutter::EncodableValue(uri));
      });
  pairing_presentation_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(),
          is_protected_pairing_context_
              ? "com.copypaste.app/pairing_presentation_context"
              : "com.copypaste.app/pairing_presentation_host",
          &flutter::StandardMethodCodec::GetInstance());
  pairing_presentation_channel_->SetMethodCallHandler(
      [this](const auto& call, auto result) {
        if (is_protected_pairing_context_) {
          if (call.method_name() == "isContextActive") {
            result->Success(flutter::EncodableValue(IsProtectedPairingContextActive()));
          } else if (call.method_name() == "ceremonyId" && IsProtectedPairingContextActive()) {
            result->Success(flutter::EncodableValue(pairing_ceremony_id_));
          } else if (call.method_name() == "status") {
            const auto status = ResolveProtectedPairingSymbol<
                decltype(&copypaste_flutter_protected_pairing_status)>(
                    "copypaste_flutter_protected_pairing_status");
            copypaste_flutter_protected_pairing_status_value value{};
            if (status == nullptr || !status(pairing_context_id_.c_str(), pairing_generation_, &value)) {
              result->Error("context_inactive");
            } else {
              flutter::EncodableMap response;
              response[flutter::EncodableValue("state")] = flutter::EncodableValue(static_cast<int32_t>(value.state));
              response[flutter::EncodableValue("expiresInMs")] = flutter::EncodableValue(static_cast<int64_t>(value.expires_in_ms));
              result->Success(flutter::EncodableValue(response));
            }
          } else if (call.method_name() == "revealQr") {
            const auto reveal = ResolveProtectedPairingSymbol<
                decltype(&copypaste_flutter_reveal_protected_pairing_qr_png)>(
                    "copypaste_flutter_reveal_protected_pairing_qr_png");
            copypaste_flutter_protected_pairing_buffer output{};
            const auto copy = reveal == nullptr ? std::nullopt
                : (reveal(pairing_ceremony_id_.c_str(), pairing_context_id_.c_str(), pairing_generation_, &output)
                    ? CopyAndReleaseBuffer(output) : std::nullopt);
            copy.has_value() ? result->Success(flutter::EncodableValue(*copy)) : result->Error("operation_rejected");
          } else if (call.method_name() == "revealSas") {
            const auto reveal = ResolveProtectedPairingSymbol<
                decltype(&copypaste_flutter_reveal_protected_pairing_sas)>(
                    "copypaste_flutter_reveal_protected_pairing_sas");
            copypaste_flutter_protected_pairing_buffer output{};
            const auto copy = reveal == nullptr ? std::nullopt
                : (reveal(pairing_context_id_.c_str(), pairing_generation_, &output)
                    ? CopyAndReleaseBuffer(output) : std::nullopt);
            copy.has_value() ? result->Success(flutter::EncodableValue(*copy)) : result->Error("operation_rejected");
          } else if (call.method_name() == "joinManual") {
            const auto* code = StringArgument(call, "code");
            const auto* address = StringArgument(call, "address");
            const auto join = ResolveProtectedPairingSymbol<
                decltype(&copypaste_flutter_protected_pairing_join)>(
                    "copypaste_flutter_protected_pairing_join");
            if (code == nullptr || code->empty() || address == nullptr || address->empty() || join == nullptr) result->Error("invalid_arguments");
            else result->Success(flutter::EncodableValue(join(pairing_context_id_.c_str(), pairing_generation_, code->c_str(), address->c_str())));
          } else if (call.method_name() == "joinQr") {
            const auto* uri = StringArgument(call, "uri");
            const auto join = ResolveProtectedPairingSymbol<
                decltype(&copypaste_flutter_protected_pairing_join_uri)>(
                    "copypaste_flutter_protected_pairing_join_uri");
            if (uri == nullptr || uri->empty() || join == nullptr) result->Error("invalid_arguments");
            else result->Success(flutter::EncodableValue(join(pairing_context_id_.c_str(), pairing_generation_, uri->c_str())));
          } else if (call.method_name() == "confirm") {
            const auto* sas = StringArgument(call, "sas");
            const auto* accept = BoolArgument(call, "accept");
            const auto decide = ResolveProtectedPairingSymbol<
                decltype(&copypaste_flutter_protected_pairing_decide)>(
                    "copypaste_flutter_protected_pairing_decide");
            if (sas == nullptr || sas->empty() || decide == nullptr) result->Error("invalid_arguments");
            else if (accept == nullptr) result->Error("invalid_arguments");
            else result->Success(flutter::EncodableValue(decide(pairing_context_id_.c_str(), pairing_generation_, sas->c_str(), *accept)));
          } else if (call.method_name() == "cancel") {
            result->Success(flutter::EncodableValue(CancelProtectedPairingContext()));
          } else if (call.method_name() == "closeContext" && CancelProtectedPairingContext()) {
            pairing_cancellation_approved_ = true;
            result->Success(flutter::EncodableValue(true));
            PostMessage(GetHandle(), WM_CLOSE, 0, 0);
          } else {
            result->NotImplemented();
          }
          return;
        }
        if (call.method_name() == "isSupported") {
          result->Success(flutter::EncodableValue(true));
          return;
        }
        if (call.method_name() == "setCaptureProtection") {
          const auto* enabled = BoolArgument(call, "enabled");
          if (enabled == nullptr) {
            result->Error("invalid_arguments");
          } else {
            result->Success(flutter::EncodableValue(SetCaptureProtection(*enabled)));
          }
          return;
        }
        if (call.method_name() != "open" && call.method_name() != "close") {
          result->NotImplemented();
          return;
        }
        const auto* arguments = std::get_if<flutter::EncodableMap>(call.arguments());
        if (arguments == nullptr) {
          result->Error("invalid_arguments");
          return;
        }
        const std::string argument_name = call.method_name() == "open"
            ? "ceremonyId"
            : "contextId";
        const auto iterator = arguments->find(flutter::EncodableValue(argument_name));
        if (iterator == arguments->end()) {
          result->Error("invalid_arguments");
          return;
        }
        const auto* value = std::get_if<std::string>(&iterator->second);
        if (value == nullptr || value->empty()) {
          result->Error("invalid_arguments");
          return;
        }
        if (call.method_name() == "close") {
          CloseProtectedPairingContext(*value);
          result->Success(flutter::EncodableValue(true));
          return;
        }
        const auto context_id = OpenProtectedPairingContext(*value);
        if (context_id.empty()) {
          result->Error("protected_context_unavailable");
          return;
        }
        flutter::EncodableMap response;
        response[flutter::EncodableValue("contextId")] =
            flutter::EncodableValue(context_id);
        result->Success(flutter::EncodableValue(response));
      });
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  if (is_quick_paste_context_) {
    const auto handle = GetHandle();
    auto style = ::GetWindowLongPtr(handle, GWL_STYLE);
    style &= ~(WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU);
    style |= WS_POPUP | WS_THICKFRAME;
    ::SetWindowLongPtr(handle, GWL_STYLE, style);
    ::SetWindowPos(handle, HWND_TOPMOST, 0, 0, 0, 0,
                   SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_NOACTIVATE);
  }

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    if (!is_quick_paste_context_) this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  CloseProtectedPairingContext(pairing_context_id_);
  CloseQuickPasteContext();
  DetachProtectedPairingContext();
  SetCaptureProtection(false);
  pairing_presentation_channel_.reset();
  pairing_links_channel_.reset();
  quick_paste_channel_.reset();
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

bool FlutterWindow::SetCaptureProtection(bool enabled) {
  const DWORD affinity = enabled ? WDA_EXCLUDEFROMCAPTURE : WDA_NONE;
  return SetWindowDisplayAffinity(GetHandle(), affinity) != FALSE;
}

bool FlutterWindow::BeginProtectedPairingContext() {
  const auto begin = ResolveProtectedPairingSymbol<
      decltype(&copypaste_flutter_begin_protected_pairing_context)>(
          "copypaste_flutter_begin_protected_pairing_context");
  pairing_generation_ = begin == nullptr ? 0
      : begin(pairing_ceremony_id_.c_str(), pairing_context_id_.c_str());
  return pairing_generation_ != 0;
}

bool FlutterWindow::IsProtectedPairingContextActive() const {
  const auto active = ResolveProtectedPairingSymbol<
      decltype(&copypaste_flutter_protected_pairing_context_active)>(
          "copypaste_flutter_protected_pairing_context_active");
  return active != nullptr && active(pairing_context_id_.c_str());
}

bool FlutterWindow::DetachProtectedPairingContext() {
  const auto detach = ResolveProtectedPairingSymbol<
      decltype(&copypaste_flutter_detach_protected_pairing_context)>(
          "copypaste_flutter_detach_protected_pairing_context");
  return detach != nullptr && detach(pairing_context_id_.c_str());
}

bool FlutterWindow::CancelProtectedPairingContext() {
  const auto cancel = ResolveProtectedPairingSymbol<
      decltype(&copypaste_flutter_cancel_protected_pairing_context)>(
          "copypaste_flutter_cancel_protected_pairing_context");
  return cancel != nullptr && cancel(pairing_context_id_.c_str());
}

std::string FlutterWindow::OpenProtectedPairingContext(
    const std::string& ceremony_id) {
  if (is_protected_pairing_context_ || ceremony_id.empty()) {
    return "";
  }
  CloseProtectedPairingContext("");
  GUID guid;
  if (CoCreateGuid(&guid) != S_OK) {
    return "";
  }
  wchar_t guid_buffer[40] = {};
  if (StringFromGUID2(guid, guid_buffer, 40) == 0) {
    return "";
  }
  const std::wstring wide_context_id(guid_buffer);
  const std::string context_id(wide_context_id.begin(), wide_context_id.end());
  flutter::DartProject project(L"data");
  project.set_dart_entrypoint_arguments(
      std::vector<std::string>{"--route=" + std::string("/protected-pairing/") + context_id});
  auto protected_window = std::make_unique<FlutterWindow>(
      project, true, context_id, ceremony_id, "", false);
  if (!protected_window->Create(L"CopyPaste", Win32Window::Point(60, 60),
                                Win32Window::Size(520, 640))) {
    return "";
  }
  protected_window->SetQuitOnClose(false);
  protected_pairing_window_ = std::move(protected_window);
  return context_id;
}

bool FlutterWindow::PrepareQuickPasteContext() {
  if (is_protected_pairing_context_ || is_quick_paste_context_) return false;
  if (quick_paste_window_) return true;
  flutter::DartProject project(L"data");
  project.set_dart_entrypoint("quickPasteMain");
  auto quick_window = std::make_unique<FlutterWindow>(
      project, false, "", "", "", true);
  if (!quick_window->Create(L"CopyPaste Quick Paste",
                            Win32Window::Point(0, 0),
                            Win32Window::Size(520, 720))) {
    return false;
  }
  quick_window->SetQuitOnClose(false);
  quick_window->main_window_handle_ = GetHandle();
  quick_paste_window_ = std::move(quick_window);
  return true;
}

bool FlutterWindow::OpenQuickPasteContext() {
  if (!PrepareQuickPasteContext()) return false;
  quick_paste_window_->previous_foreground_window_ = ::GetForegroundWindow();
  quick_paste_window_->ShowQuickPasteAtCursor();
  return true;
}

void FlutterWindow::CloseQuickPasteContext() {
  if (is_quick_paste_context_ || !quick_paste_window_) return;
  quick_paste_window_->Destroy();
  quick_paste_window_.reset();
}

void FlutterWindow::ShowQuickPasteAtCursor() {
  if (!is_quick_paste_context_) return;
  POINT cursor{};
  if (!::GetCursorPos(&cursor)) return;
  const HMONITOR monitor = ::MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
  MONITORINFO monitor_info{sizeof(MONITORINFO)};
  if (!::GetMonitorInfo(monitor, &monitor_info)) return;
  // Move first so GetDpiForWindow resolves the display under the pointer rather
  // than the display where this pre-warmed hidden window was created.
  ::SetWindowPos(GetHandle(), HWND_TOPMOST, cursor.x, cursor.y, 0, 0,
                 SWP_NOSIZE | SWP_NOACTIVATE);
  const UINT dpi = ::GetDpiForWindow(GetHandle());
  const int width = ::MulDiv(520, dpi, USER_DEFAULT_SCREEN_DPI);
  const int height = ::MulDiv(720, dpi, USER_DEFAULT_SCREEN_DPI);
  const RECT work = monitor_info.rcWork;
  const int maximum_x = std::max(work.left, work.right - width);
  const int maximum_y = std::max(work.top, work.bottom - height);
  const int x = std::clamp(static_cast<int>(cursor.x), work.left, maximum_x);
  const int y = std::clamp(static_cast<int>(cursor.y) + 8, work.top, maximum_y);
  ::SetWindowPos(GetHandle(), HWND_TOPMOST, x, y, width, height,
                 SWP_SHOWWINDOW | SWP_FRAMECHANGED);
  ::SetForegroundWindow(GetHandle());
  if (quick_paste_channel_) {
    quick_paste_channel_->InvokeMethod(
        "opened", std::make_unique<flutter::EncodableValue>());
  }
}

bool FlutterWindow::PasteIntoPreviousWindow() {
  if (!is_quick_paste_context_ || previous_foreground_window_ == nullptr ||
      !::IsWindow(previous_foreground_window_)) {
    return false;
  }
  ::ShowWindow(GetHandle(), SW_HIDE);
  ::SetForegroundWindow(previous_foreground_window_);
  INPUT inputs[4]{};
  inputs[0].type = INPUT_KEYBOARD;
  inputs[0].ki.wVk = VK_CONTROL;
  inputs[1].type = INPUT_KEYBOARD;
  inputs[1].ki.wVk = 'V';
  inputs[2].type = INPUT_KEYBOARD;
  inputs[2].ki.wVk = 'V';
  inputs[2].ki.dwFlags = KEYEVENTF_KEYUP;
  inputs[3].type = INPUT_KEYBOARD;
  inputs[3].ki.wVk = VK_CONTROL;
  inputs[3].ki.dwFlags = KEYEVENTF_KEYUP;
  return ::SendInput(4, inputs, sizeof(INPUT)) == 4;
}

void FlutterWindow::ShowMainWindow(bool open_settings) {
  if (!is_quick_paste_context_ || main_window_handle_ == nullptr) return;
  ::ShowWindow(GetHandle(), SW_HIDE);
  ::ShowWindow(main_window_handle_, SW_SHOW);
  ::SetForegroundWindow(main_window_handle_);
  if (open_settings && quick_paste_channel_) {
    // The context asks its owner to route Settings on the main Flutter engine.
    ::PostMessage(main_window_handle_, WM_APP + 41, 0, 0);
  }
}

void FlutterWindow::CloseProtectedPairingContext(const std::string& context_id) {
  if (is_protected_pairing_context_ || !protected_pairing_window_) {
    return;
  }
  if (!context_id.empty() &&
      protected_pairing_window_->pairing_context_id_ != context_id) {
    return;
  }
  protected_pairing_window_->Destroy();
  protected_pairing_window_.reset();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  if (is_protected_pairing_context_ && message == WM_CLOSE && !pairing_cancellation_approved_) {
    pairing_cancellation_approved_ = CancelProtectedPairingContext();
    if (!pairing_cancellation_approved_) return 0;
  }
  if (is_quick_paste_context_) {
    if (message == WM_CLOSE) {
      ::ShowWindow(hwnd, SW_HIDE);
      return 0;
    }
    if (message == WM_ACTIVATE && LOWORD(wparam) == WA_INACTIVE) {
      ::ShowWindow(hwnd, SW_HIDE);
      return 0;
    }
  } else if (!is_protected_pairing_context_ && message == WM_APP + 41) {
    if (quick_paste_channel_) {
      quick_paste_channel_->InvokeMethod(
          "openSettings", std::make_unique<flutter::EncodableValue>());
    }
    return 0;
  }
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
