#include "startup_channel.h"

#include <windows.h>
#include <flutter/standard_method_codec.h>
#include <optional>
#include <string>
#include <vector>

namespace {
constexpr wchar_t kRunKey[] = L"Software\\Microsoft\\Windows\\CurrentVersion\\Run";
constexpr wchar_t kRunValue[] = L"CopyPaste";

std::optional<std::wstring> StartupCommand() {
  std::vector<wchar_t> path(32768);
  const DWORD length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
  if (length == 0 || length >= path.size()) return std::nullopt;
  return L"\"" + std::wstring(path.data(), length) + L"\"";
}

std::optional<bool> Enabled() {
  const auto command = StartupCommand();
  if (!command) return std::nullopt;
  DWORD bytes = 0;
  LSTATUS status = RegGetValueW(HKEY_CURRENT_USER, kRunKey, kRunValue,
      RRF_RT_REG_SZ, nullptr, nullptr, &bytes);
  if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) return false;
  if (status != ERROR_SUCCESS || bytes > 65536 || bytes % sizeof(wchar_t) != 0) return std::nullopt;
  std::vector<wchar_t> value(bytes / sizeof(wchar_t) + 1, L'\0');
  status = RegGetValueW(HKEY_CURRENT_USER, kRunKey, kRunValue,
      RRF_RT_REG_SZ, nullptr, value.data(), &bytes);
  if (status != ERROR_SUCCESS) return std::nullopt;
  return std::wstring(value.data()) == *command;
}

bool SetEnabled(bool enabled) {
  const auto command = StartupCommand();
  if (!command) return false;
  if (!enabled) {
    const LSTATUS status = RegDeleteKeyValueW(HKEY_CURRENT_USER, kRunKey, kRunValue);
    return status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND;
  }
  return RegSetKeyValueW(HKEY_CURRENT_USER, kRunKey, kRunValue, REG_SZ,
      command->c_str(), static_cast<DWORD>((command->size() + 1) * sizeof(wchar_t))) == ERROR_SUCCESS;
}
}

StartupChannel::StartupChannel(flutter::BinaryMessenger* messenger) {
  channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
      messenger, "com.copypaste.app/startup", &flutter::StandardMethodCodec::GetInstance());
  channel_->SetMethodCallHandler([](const auto& call, auto result) {
    if (call.method_name() == "getEnabled") {
      const auto enabled = Enabled();
      if (enabled) result->Success(flutter::EncodableValue(*enabled));
      else result->Error("startup_unavailable", "Startup registration could not be read.");
    } else if (call.method_name() == "setEnabled") {
      const auto* arguments = call.arguments() == nullptr ? nullptr
          : std::get_if<flutter::EncodableMap>(call.arguments());
      const auto found = arguments == nullptr ? flutter::EncodableMap::const_iterator{}
          : arguments->find(flutter::EncodableValue("enabled"));
      const auto* enabled = arguments == nullptr || found == arguments->end()
          ? nullptr : std::get_if<bool>(&found->second);
      if (enabled == nullptr) result->Error("invalid_arguments");
      else if (!SetEnabled(*enabled)) result->Error("startup_update_failed");
      else result->Success(flutter::EncodableValue(true));
    } else {
      result->NotImplemented();
    }
  });
}

StartupChannel::~StartupChannel() {
  channel_->SetMethodCallHandler(nullptr);
}
