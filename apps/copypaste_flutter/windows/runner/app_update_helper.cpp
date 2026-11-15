#include <windows.h>
#include <shellapi.h>

#include <algorithm>
#include <cstdlib>
#include <cwchar>
#include <string>
#include <vector>
#include <optional>

#include "app_update_verifier.h"

namespace {

std::optional<std::wstring> ArgumentValue(
    const std::vector<std::wstring>& arguments, const std::wstring& prefix) {
  for (const auto& argument : arguments) {
    if (argument.rfind(prefix, 0) == 0) return argument.substr(prefix.size());
  }
  return std::nullopt;
}

}  // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
  int count = 0;
  wchar_t** raw = ::CommandLineToArgvW(::GetCommandLineW(), &count);
  if (raw == nullptr) return EXIT_FAILURE;
  const std::vector<std::wstring> arguments(raw + 1, raw + count);
  ::LocalFree(raw);
  if (std::find(arguments.begin(), arguments.end(),
                L"--copypaste-update-helper") == arguments.end()) {
    return EXIT_FAILURE;
  }
  const auto parent_value = ArgumentValue(arguments, L"--parent-pid=");
  const auto installer_value = ArgumentValue(arguments, L"--installer=");
  const auto sha256 = ArgumentValue(arguments, L"--sha256=");
  if (!parent_value.has_value() || !installer_value.has_value() ||
      !sha256.has_value()) {
    return EXIT_FAILURE;
  }
  wchar_t* end = nullptr;
  const unsigned long parsed_pid =
      std::wcstoul(parent_value->c_str(), &end, 10);
  if (end == parent_value->c_str() || *end != L'\0' || parsed_pid == 0 ||
      parsed_pid > MAXDWORD) {
    return EXIT_FAILURE;
  }
  const std::wstring& installer = *installer_value;
  if (installer.empty() || sha256->size() != 64) return EXIT_FAILURE;
  std::string expected_sha256;
  expected_sha256.reserve(64);
  for (const wchar_t digit : *sha256) {
    if (!((digit >= L'0' && digit <= L'9') ||
          (digit >= L'a' && digit <= L'f'))) return EXIT_FAILURE;
    expected_sha256.push_back(static_cast<char>(digit));
  }

  const HANDLE parent =
      ::OpenProcess(SYNCHRONIZE, FALSE, static_cast<DWORD>(parsed_pid));
  if (parent == nullptr) return EXIT_FAILURE;
  const DWORD wait = ::WaitForSingleObject(parent, 60 * 1000);
  ::CloseHandle(parent);
  if (wait != WAIT_OBJECT_0 || !VerifyCopyPasteInstaller(installer, expected_sha256)) {
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
