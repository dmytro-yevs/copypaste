#ifndef FLUTTER_PLUGIN_HOTKEY_MANAGER_WINDOWS_PLUGIN_H_
#define FLUTTER_PLUGIN_HOTKEY_MANAGER_WINDOWS_PLUGIN_H_

#include <windows.h>

#include <flutter/event_channel.h>
#include <flutter/event_stream_handler_functions.h>
#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

namespace hotkey_manager_windows {

struct HotKeyApi {
  decltype(&::RegisterHotKey) register_hot_key = &::RegisterHotKey;
  decltype(&::UnregisterHotKey) unregister_hot_key = &::UnregisterHotKey;
  decltype(&::GetLastError) get_last_error = &::GetLastError;
};

class HotkeyManagerWindowsPlugin
    : public flutter::Plugin,
      flutter::StreamHandler<flutter::EncodableValue> {
 private:
  flutter::PluginRegistrarWindows* registrar_;
  std::unique_ptr<flutter::EventSink<flutter::EncodableValue>> event_sink_;

  struct Registration {
    HWND window;
    int32_t id;
  };
  std::unordered_map<std::string, Registration> hotkey_id_map_;
  HotKeyApi api_;
  HWND test_window_ = nullptr;
  bool Release(const std::string& identifier, DWORD* error);
  HWND RegistrationWindow() const;
  int32_t window_proc_id_ = -1;
  std::optional<LRESULT> HandleWindowProc(HWND hwnd,
                                          UINT message,
                                          WPARAM wparam,
                                          LPARAM lparam);

 public:
  static void RegisterWithRegistrar(flutter::PluginRegistrarWindows* registrar);

  explicit HotkeyManagerWindowsPlugin(flutter::PluginRegistrarWindows* registrar);
  // Inject OS calls and an owned window without creating a native registrar.
  HotkeyManagerWindowsPlugin(HotKeyApi api, HWND window);

  virtual ~HotkeyManagerWindowsPlugin();

  // Disallow copy and assign.
  HotkeyManagerWindowsPlugin(const HotkeyManagerWindowsPlugin&) = delete;
  HotkeyManagerWindowsPlugin& operator=(const HotkeyManagerWindowsPlugin&) =
      delete;

  void Register(
      const flutter::MethodCall<flutter::EncodableValue>& method_call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
  void Unregister(
      const flutter::MethodCall<flutter::EncodableValue>& method_call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
  void UnregisterAll(
      const flutter::MethodCall<flutter::EncodableValue>& method_call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
  UINT GetVirtualKeyCodeFromString(const std::string key_code);
  UINT GetFsModifiersFromString(const std::vector<std::string>& modifiers);

  // Called when a method is called on this plugin's channel from Dart.
  void HandleMethodCall(
      const flutter::MethodCall<flutter::EncodableValue>& method_call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  std::unique_ptr<flutter::StreamHandlerError<>> OnListenInternal(
      const flutter::EncodableValue* arguments,
      std::unique_ptr<flutter::EventSink<>>&& events) override;

  std::unique_ptr<flutter::StreamHandlerError<>> OnCancelInternal(
      const flutter::EncodableValue* arguments) override;
};

}  // namespace hotkey_manager_windows

#endif  // FLUTTER_PLUGIN_HOTKEY_MANAGER_WINDOWS_PLUGIN_H_
