#include "app_update_channel.h"

#include <flutter/standard_method_codec.h>
#include <shellapi.h>

#include <algorithm>
#include <optional>
#include <string>
#include <vector>

#include "app_update_verifier.h"

namespace {

const std::string* StringArgument(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    const char* name) {
  const auto* arguments = std::get_if<flutter::EncodableMap>(call.arguments());
  if (arguments == nullptr) return nullptr;
  const auto iterator = arguments->find(flutter::EncodableValue(name));
  return iterator == arguments->end()
             ? nullptr
             : std::get_if<std::string>(&iterator->second);
}

std::wstring Utf16FromUtf8(const std::string& value) {
  if (value.empty()) return {};
  const int length =
      ::MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(),
                            static_cast<int>(value.size()), nullptr, 0);
  if (length <= 0) return {};
  std::wstring output(length, L'\0');
  if (::MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(),
                            static_cast<int>(value.size()), output.data(),
                            length) != length) {
    return {};
  }
  return output;
}

std::optional<std::wstring> CurrentExecutable() {
  std::vector<wchar_t> path(32768);
  const DWORD length = ::GetModuleFileNameW(nullptr, path.data(),
                                            static_cast<DWORD>(path.size()));
  if (length == 0 || length >= path.size()) return std::nullopt;
  return std::wstring(path.data(), length);
}

bool IsTrustedReleaseUrl(const std::string& value) {
  const std::string prefix =
      "https://github.com/dmytro-yevs/copypaste/releases/";
  return value.rfind(prefix, 0) == 0;
}

bool LaunchHelper(const std::wstring& installer, const std::string& sha256) {
  const auto current = CurrentExecutable();
  if (!current.has_value()) return false;
  wchar_t temporary_path[MAX_PATH] = {};
  const DWORD temporary_length = ::GetTempPathW(MAX_PATH, temporary_path);
  if (temporary_length == 0 || temporary_length >= MAX_PATH) return false;
  const std::wstring helper =
      std::wstring(temporary_path) + L"CopyPaste-update-helper-" +
      std::to_wstring(::GetCurrentProcessId()) + L".exe";
  ::DeleteFileW(helper.c_str());
  if (!::CopyFileW(current->c_str(), helper.c_str(), TRUE)) return false;

  std::wstring command =
      L"\"" + helper + L"\" --copypaste-update-helper --parent-pid=" +
      std::to_wstring(::GetCurrentProcessId()) + L" --installer=\"" +
      installer + L"\" --sha256=" + Utf16FromUtf8(sha256);
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  const BOOL created =
      ::CreateProcessW(helper.c_str(), command.data(), nullptr, nullptr, FALSE,
                       CREATE_NO_WINDOW | DETACHED_PROCESS, nullptr,
                       temporary_path, &startup, &process);
  if (!created) {
    ::DeleteFileW(helper.c_str());
    return false;
  }
  ::CloseHandle(process.hThread);
  ::CloseHandle(process.hProcess);
  return true;
}

}  // namespace

AppUpdateChannel::AppUpdateChannel(flutter::BinaryMessenger* messenger,
                                   HWND window)
    : window_(window),
      channel_(
          std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
              messenger, "com.copypaste.app/app_update",
              &flutter::StandardMethodCodec::GetInstance())) {
  channel_->SetMethodCallHandler([this](const auto& call, auto result) {
    if (call.method_name() == "currentVersion") {
      result->Success(flutter::EncodableValue(FLUTTER_VERSION));
      return;
    }
    if (call.method_name() == "systemVersion") {
      using RtlGetVersionFunction = LONG(WINAPI*)(OSVERSIONINFOW*);
      auto* ntdll = ::GetModuleHandleW(L"ntdll.dll");
      auto get_version = ntdll == nullptr ? nullptr :
          reinterpret_cast<RtlGetVersionFunction>(::GetProcAddress(ntdll, "RtlGetVersion"));
      OSVERSIONINFOW version{};
      version.dwOSVersionInfoSize = sizeof(version);
      if (get_version == nullptr || get_version(&version) != 0) {
        result->Error("system_version_unavailable", "System version is unavailable.");
        return;
      }
      result->Success(flutter::EncodableValue(
          std::to_string(version.dwMajorVersion) + "." +
          std::to_string(version.dwMinorVersion) + "." +
          std::to_string(version.dwBuildNumber)));
      return;
    }
    if (call.method_name() == "availability") {
      const bool available = CurrentCopyPasteExecutableIsSigned();
      flutter::EncodableMap response;
      response[flutter::EncodableValue("available")] =
          flutter::EncodableValue(available);
      if (!available) {
        response[flutter::EncodableValue("reason")] = flutter::EncodableValue(
            "Install a signed CopyPaste release to update it here.");
      }
      result->Success(flutter::EncodableValue(response));
      return;
    }
    if (call.method_name() == "openReleasePage") {
      const auto* url = StringArgument(call, "url");
      if (url == nullptr || !IsTrustedReleaseUrl(*url)) {
        result->Error("invalid_arguments");
        return;
      }
      const std::wstring wide_url = Utf16FromUtf8(*url);
      const auto launched = reinterpret_cast<INT_PTR>(::ShellExecuteW(
          window_, L"open", wide_url.c_str(), nullptr, nullptr, SW_SHOWNORMAL));
      launched > 32 ? result->Success() : result->Error("open_failed");
      return;
    }
    if (call.method_name() != "install") {
      result->NotImplemented();
      return;
    }
    const auto* path = StringArgument(call, "path");
    const auto* sha256 = StringArgument(call, "sha256");
    if (path == nullptr || sha256 == nullptr) {
      result->Error("invalid_arguments");
      return;
    }
    const std::wstring installer = Utf16FromUtf8(*path);
    if (installer.empty() || !VerifyCopyPasteInstaller(installer, *sha256)) {
      result->Error("signature_invalid");
      return;
    }
    if (!LaunchHelper(installer, *sha256)) {
      result->Error("installer_launch_failed");
      return;
    }
    result->Success(flutter::EncodableValue("started"));
    ::PostQuitMessage(0);
  });
}

AppUpdateChannel::~AppUpdateChannel() {
  channel_->SetMethodCallHandler(nullptr);
}
