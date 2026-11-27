#include "app_update_channel.h"

#include <flutter/standard_method_codec.h>
#include <netlistmgr.h>
#include <shellapi.h>
#include <wrl/client.h>

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

bool IsTrustedReleaseUrl(const std::string& value) {
  const std::string prefix =
      "https://github.com/dmytro-yevs/copypaste/releases/";
  return value.rfind(prefix, 0) == 0;
}

// Starts the verified installer in its own visible window. The installer waits
// for this process and its daemon to exit before it replaces their files, and
// runs from the download directory so it does not hold the install directory.
bool LaunchInstaller(const std::wstring& installer) {
  const auto separator = installer.find_last_of(L"\\/");
  const std::wstring directory = separator == std::wstring::npos
                                     ? std::wstring()
                                     : installer.substr(0, separator);
  const auto launched = reinterpret_cast<INT_PTR>(::ShellExecuteW(
      nullptr, L"open", installer.c_str(), nullptr,
      directory.empty() ? nullptr : directory.c_str(), SW_SHOWNORMAL));
  return launched > 32;
}

}  // namespace

// Battery saver and a metered (roaming, capped or variable-cost) connection
// both defer a background download; an unknown answer allows it.
static bool BackgroundDownloadAllowed() {
  SYSTEM_POWER_STATUS power{};
  if (::GetSystemPowerStatus(&power) && power.SystemStatusFlag == 1) return false;
  Microsoft::WRL::ComPtr<INetworkCostManager> costs;
  if (FAILED(::CoCreateInstance(CLSID_NetworkListManager, nullptr, CLSCTX_ALL,
                                IID_PPV_ARGS(&costs)))) {
    return true;
  }
  DWORD cost = 0;
  if (FAILED(costs->GetCost(&cost, nullptr))) return true;
  return (cost & NLM_CONNECTION_COST_UNRESTRICTED) != 0 ||
         cost == NLM_CONNECTION_COST_UNKNOWN;
}

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
    if (call.method_name() == "backgroundDownloadAllowed") {
      result->Success(flutter::EncodableValue(BackgroundDownloadAllowed()));
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
    if (!LaunchInstaller(installer)) {
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
