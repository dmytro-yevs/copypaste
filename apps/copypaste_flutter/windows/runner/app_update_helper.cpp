#include "app_update_helper.h"

#include <windows.h>
#include <shellapi.h>

#include <algorithm>
#include <cstdlib>
#include <optional>

#include "app_update_verifier.h"

namespace {

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

std::optional<std::string> ArgumentValue(
    const std::vector<std::string>& arguments, const std::string& prefix) {
  for (const auto& argument : arguments) {
    if (argument.rfind(prefix, 0) == 0) return argument.substr(prefix.size());
  }
  return std::nullopt;
}

}  // namespace

int RunAppUpdateHelperIfRequested(const std::vector<std::string>& arguments) {
  if (std::find(arguments.begin(), arguments.end(),
                "--copypaste-update-helper") == arguments.end()) {
    return -1;
  }
  const auto parent_value = ArgumentValue(arguments, "--parent-pid=");
  const auto installer_value = ArgumentValue(arguments, "--installer=");
  const auto sha256 = ArgumentValue(arguments, "--sha256=");
  if (!parent_value.has_value() || !installer_value.has_value() ||
      !sha256.has_value()) {
    return EXIT_FAILURE;
  }
  char* end = nullptr;
  const unsigned long parsed_pid =
      std::strtoul(parent_value->c_str(), &end, 10);
  if (end == parent_value->c_str() || *end != '\0' || parsed_pid == 0 ||
      parsed_pid > MAXDWORD) {
    return EXIT_FAILURE;
  }
  const std::wstring installer = Utf16FromUtf8(*installer_value);
  if (installer.empty()) return EXIT_FAILURE;

  const HANDLE parent =
      ::OpenProcess(SYNCHRONIZE, FALSE, static_cast<DWORD>(parsed_pid));
  if (parent == nullptr) return EXIT_FAILURE;
  const DWORD wait = ::WaitForSingleObject(parent, 60 * 1000);
  ::CloseHandle(parent);
  if (wait != WAIT_OBJECT_0 || !VerifyCopyPasteInstaller(installer, *sha256)) {
    return EXIT_FAILURE;
  }

  const auto launched = reinterpret_cast<INT_PTR>(::ShellExecuteW(
      nullptr, L"open", installer.c_str(), nullptr, nullptr, SW_SHOWNORMAL));
  wchar_t self[32768] = {};
  const DWORD length = ::GetModuleFileNameW(nullptr, self, 32768);
  if (length > 0 && length < 32768) {
    ::MoveFileExW(self, nullptr, MOVEFILE_DELAY_UNTIL_REBOOT);
  }
  return launched > 32 ? EXIT_SUCCESS : EXIT_FAILURE;
}
