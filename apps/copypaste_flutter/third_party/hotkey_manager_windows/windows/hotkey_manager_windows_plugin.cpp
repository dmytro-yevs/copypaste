#include "hotkey_manager_windows_plugin.h"

// This must be included before many other Windows headers.
#include <windows.h>

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>

#include <memory>
#include <sstream>
#include <unordered_set>

namespace hotkey_manager_windows {
namespace {
std::unordered_set<int32_t> allocated_ids;
int32_t next_id = 0;

int32_t AllocateId() {
  for (int32_t count = 0; count < 0xBFFF; ++count) {
    next_id = next_id == 0xBFFF ? 1 : next_id + 1;
    if (allocated_ids.insert(next_id).second) return next_id;
  }
  return 0;
}

const flutter::EncodableValue* Argument(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    const char* name) {
  if (!call.arguments()) return nullptr;
  const auto* map = std::get_if<flutter::EncodableMap>(call.arguments());
  if (!map) return nullptr;
  const auto found = map->find(flutter::EncodableValue(name));
  return found == map->end() ? nullptr : &found->second;
}

const std::string* Identifier(
    const flutter::MethodCall<flutter::EncodableValue>& call) {
  const auto* value = Argument(call, "identifier");
  return value ? std::get_if<std::string>(value) : nullptr;
}

flutter::EncodableValue ErrorDetails(DWORD error) {
  return flutter::EncodableValue(flutter::EncodableMap{
      {flutter::EncodableValue("win32Error"),
       flutter::EncodableValue(static_cast<int64_t>(error))}});
}
}  // namespace


// static
void HotkeyManagerWindowsPlugin::RegisterWithRegistrar(
    flutter::PluginRegistrarWindows* registrar) {
  auto channel =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          registrar->messenger(), "dev.leanflutter.plugins/hotkey_manager",
          &flutter::StandardMethodCodec::GetInstance());

  auto plugin = std::make_unique<HotkeyManagerWindowsPlugin>(registrar);

  channel->SetMethodCallHandler(
      [plugin_pointer = plugin.get()](const auto& call, auto result) {
        plugin_pointer->HandleMethodCall(call, std::move(result));
      });

  auto event_channel =
      std::make_unique<flutter::EventChannel<flutter::EncodableValue>>(
          registrar->messenger(),
          "dev.leanflutter.plugins/hotkey_manager_event",
          &flutter::StandardMethodCodec::GetInstance());
  auto streamHandler = std::make_unique<flutter::StreamHandlerFunctions<>>(
      [plugin_pointer = plugin.get()](
          const flutter::EncodableValue* arguments,
          std::unique_ptr<flutter::EventSink<>>&& events)
          -> std::unique_ptr<flutter::StreamHandlerError<>> {
        return plugin_pointer->OnListen(arguments, std::move(events));
      },
      [plugin_pointer = plugin.get()](const flutter::EncodableValue* arguments)
          -> std::unique_ptr<flutter::StreamHandlerError<>> {
        return plugin_pointer->OnCancel(arguments);
      });
  event_channel->SetStreamHandler(std::move(streamHandler));

  registrar->AddPlugin(std::move(plugin));
}

HotkeyManagerWindowsPlugin::HotkeyManagerWindowsPlugin(
    flutter::PluginRegistrarWindows* registrar) {
  registrar_ = registrar;
  window_proc_id_ = registrar->RegisterTopLevelWindowProcDelegate(
      [this](HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
        return HandleWindowProc(hwnd, message, wparam, lparam);
      });
}

HotkeyManagerWindowsPlugin::HotkeyManagerWindowsPlugin(HotKeyApi api, HWND window)
    : registrar_(nullptr), api_(api), test_window_(window) {}

HotkeyManagerWindowsPlugin::~HotkeyManagerWindowsPlugin() {
  event_sink_.reset();
  if (registrar_ && window_proc_id_ >= 0) {
    registrar_->UnregisterTopLevelWindowProcDelegate(window_proc_id_);
  }
  for (const auto& entry : hotkey_id_map_) {
    if (api_.unregister_hot_key(entry.second.window, entry.second.id)) {
      allocated_ids.erase(entry.second.id);
    }
  }
}

HWND HotkeyManagerWindowsPlugin::RegistrationWindow() const {
  if (!registrar_) return test_window_;
  if (!registrar_->GetView()) return nullptr;
  return ::GetAncestor(registrar_->GetView()->GetNativeWindow(), GA_ROOT);
}

bool HotkeyManagerWindowsPlugin::Release(const std::string& identifier,
                                         DWORD* error) {
  const auto found = hotkey_id_map_.find(identifier);
  if (found == hotkey_id_map_.end()) return true;
  if (!api_.unregister_hot_key(found->second.window, found->second.id)) {
    *error = api_.get_last_error();
    return false;
  }
  allocated_ids.erase(found->second.id);
  hotkey_id_map_.erase(found);
  return true;
}

std::optional<LRESULT> HotkeyManagerWindowsPlugin::HandleWindowProc(
    HWND hwnd,
    UINT message,
    WPARAM wparam,
    LPARAM lparam) {
  switch (message) {
    case WM_HOTKEY: {
      int32_t hotkey_id = static_cast<int32_t>(wparam);
      for (const auto& [identifier, registration] : hotkey_id_map_) {
        if (registration.id == hotkey_id && registration.window == hwnd) {
          flutter::EncodableMap args = flutter::EncodableMap();
          args[flutter::EncodableValue("type")] = "onKeyDown";
          args[flutter::EncodableValue("data")] =
              flutter::EncodableMap({{"identifier", identifier}});
          if (event_sink_) {
            event_sink_->Success(flutter::EncodableValue(args));
          }
          break;
        }
      }
    }
  }
  return std::nullopt;
}

void HotkeyManagerWindowsPlugin::Register(
    const flutter::MethodCall<flutter::EncodableValue>& method_call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  const auto* identifier = Identifier(method_call);
  const auto* key_value = Argument(method_call, "keyCode");
  const auto* key = key_value ? std::get_if<int32_t>(key_value) : nullptr;
  const auto* modifiers_value = Argument(method_call, "modifiers");
  const auto* list = modifiers_value
      ? std::get_if<flutter::EncodableList>(modifiers_value) : nullptr;
  if (!identifier || identifier->empty() || !key || *key < 1 || *key > 255 ||
      !list) {
    result->Error("invalid_hotkey_arguments", "Invalid hotkey registration arguments.");
    return;
  }
  std::vector<std::string> modifiers;
  for (const auto& value : *list) {
    const auto* modifier = std::get_if<std::string>(&value);
    if (!modifier || (*modifier != "alt" && *modifier != "control" &&
                      *modifier != "meta" && *modifier != "shift")) {
      result->Error("invalid_hotkey_arguments", "Invalid hotkey modifier.");
      return;
    }
    modifiers.push_back(*modifier);
  }
  DWORD error = ERROR_SUCCESS;
  if (!Release(*identifier, &error)) {
    result->Error("hotkey_unregistration_failed", "Windows could not release the global shortcut.", ErrorDetails(error));
    return;
  }
  const HWND window = RegistrationWindow();
  if (!window) {
    result->Error("hotkey_registration_failed", "The registering window is unavailable.");
    return;
  }
  const int32_t id = AllocateId();
  if (id == 0) {
    result->Error("hotkey_registration_failed", "No hotkey identifier is available.");
    return;
  }
  if (!api_.register_hot_key(window, id, GetFsModifiersFromString(modifiers), *key)) {
    error = api_.get_last_error();
    allocated_ids.erase(id);
    result->Error("hotkey_registration_failed", "Windows rejected the global shortcut registration.", ErrorDetails(error));
    return;
  }
  hotkey_id_map_.emplace(*identifier, Registration{window, id});
  result->Success(flutter::EncodableValue(true));
}

void HotkeyManagerWindowsPlugin::Unregister(
    const flutter::MethodCall<flutter::EncodableValue>& method_call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  const auto* identifier = Identifier(method_call);
  if (!identifier || identifier->empty()) {
    result->Error("invalid_hotkey_arguments", "The hotkey identifier is missing.");
    return;
  }
  DWORD error = ERROR_SUCCESS;
  if (!Release(*identifier, &error)) {
    result->Error("hotkey_unregistration_failed", "Windows could not release the global shortcut.", ErrorDetails(error));
    return;
  }
  result->Success(flutter::EncodableValue(true));
}

void HotkeyManagerWindowsPlugin::UnregisterAll(
    const flutter::MethodCall<flutter::EncodableValue>& method_call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  std::vector<std::string> identifiers;
  for (const auto& entry : hotkey_id_map_) identifiers.push_back(entry.first);
  DWORD first_error = ERROR_SUCCESS;
  bool failed = false;
  for (const auto& identifier : identifiers) {
    DWORD error = ERROR_SUCCESS;
    if (!Release(identifier, &error) && !failed) {
      first_error = error;
      failed = true;
    }
  }
  if (failed) {
    result->Error("hotkey_unregistration_failed", "Windows could not release every global shortcut.", ErrorDetails(first_error));
    return;
  }
  result->Success(flutter::EncodableValue(true));
}

UINT HotkeyManagerWindowsPlugin::GetFsModifiersFromString(
    const std::vector<std::string>& modifiers) {
  UINT fs_modifiers = 0x0000;
  for (size_t i = 0; i < modifiers.size(); i++) {
    UINT fs_modifier = 0x0000;
    if (modifiers[i] == "alt") {
      fs_modifier = MOD_ALT;
    } else if (modifiers[i] == "control") {
      fs_modifier = MOD_CONTROL;
    } else if (modifiers[i] == "meta") {
      fs_modifier = MOD_WIN;
    } else if (modifiers[i] == "shift") {
      fs_modifier = MOD_SHIFT;
    }
    fs_modifiers = fs_modifiers | fs_modifier;
  }
  return fs_modifiers;
}

void HotkeyManagerWindowsPlugin::HandleMethodCall(
    const flutter::MethodCall<flutter::EncodableValue>& method_call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  if (method_call.method_name().compare("register") == 0) {
    Register(method_call, std::move(result));
  } else if (method_call.method_name().compare("unregister") == 0) {
    Unregister(method_call, std::move(result));
  } else if (method_call.method_name().compare("unregisterAll") == 0) {
    UnregisterAll(method_call, std::move(result));
  } else {
    result->NotImplemented();
  }
}

std::unique_ptr<flutter::StreamHandlerError<flutter::EncodableValue>>
HotkeyManagerWindowsPlugin::OnListenInternal(
    const flutter::EncodableValue* arguments,
    std::unique_ptr<flutter::EventSink<flutter::EncodableValue>>&& events) {
  event_sink_ = std::move(events);
  return nullptr;
}

std::unique_ptr<flutter::StreamHandlerError<flutter::EncodableValue>>
HotkeyManagerWindowsPlugin::OnCancelInternal(
    const flutter::EncodableValue* arguments) {
  event_sink_ = nullptr;
  return nullptr;
}

}  // namespace hotkey_manager_windows
