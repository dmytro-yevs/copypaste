#include "application_instance.h"

namespace {
constexpr ULONG_PTR kLaunchData = 0x43504c31;
constexpr size_t kMaximumUriBytes = 8192;
}

ApplicationInstance::ApplicationInstance() {
  mutex_ = ::CreateMutexW(nullptr, FALSE, L"Local\\CopyPaste.MainWindow.v1");
  primary_ = mutex_ != nullptr && ::GetLastError() != ERROR_ALREADY_EXISTS;
}

ApplicationInstance::~ApplicationInstance() {
  if (mutex_ != nullptr) ::CloseHandle(mutex_);
}

bool ApplicationInstance::OpenExisting(const std::string& pairing_uri) const {
  const HWND window = ::FindWindowW(L"FLUTTER_RUNNER_WIN32_WINDOW", L"CopyPaste");
  if (window == nullptr || pairing_uri.size() > kMaximumUriBytes) return false;
  DWORD process_id = 0;
  ::GetWindowThreadProcessId(window, &process_id);
  ::AllowSetForegroundWindow(process_id);
  COPYDATASTRUCT data{};
  data.dwData = kLaunchData;
  data.cbData = static_cast<DWORD>(pairing_uri.size() + 1);
  data.lpData = const_cast<char*>(pairing_uri.c_str());
  DWORD_PTR result = 0;
  return ::SendMessageTimeoutW(window, WM_COPYDATA, 0,
      reinterpret_cast<LPARAM>(&data), SMTO_ABORTIFHUNG, 5000, &result) != 0 &&
      result != 0;
}

std::optional<std::string> ApplicationInstance::ReadLaunch(const COPYDATASTRUCT* data) {
  if (data == nullptr || data->dwData != kLaunchData || data->lpData == nullptr ||
      data->cbData == 0 || data->cbData > kMaximumUriBytes + 1) return std::nullopt;
  const auto* bytes = static_cast<const char*>(data->lpData);
  if (bytes[data->cbData - 1] != '\0') return std::nullopt;
  const std::string uri(bytes, data->cbData - 1);
  if (uri.find('\0') != std::string::npos) return std::nullopt;
  if (!uri.empty() && uri.rfind("copypaste://pair/v1?", 0) != 0 &&
      uri.rfind("copypaste://pair/v2?", 0) != 0) return std::nullopt;
  return uri;
}
