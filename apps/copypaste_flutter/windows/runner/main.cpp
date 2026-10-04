#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <algorithm>
#include <string>

#include "flutter_window.h"
#include "utils.h"

namespace {

constexpr char kPairingUriPrefix[] = "copypaste://pair/v1?";

void RegisterPairingProtocol() {
  wchar_t executable[MAX_PATH] = {};
  const DWORD length = ::GetModuleFileNameW(nullptr, executable, MAX_PATH);
  if (length == 0 || length >= MAX_PATH) return;

  const std::wstring command = L"\"" + std::wstring(executable) + L"\" \"%1\"";
  const std::wstring icon = L"\"" + std::wstring(executable) + L"\",0";
  const auto write = [](const wchar_t* key, const wchar_t* name,
                        const std::wstring& value) {
    ::RegSetKeyValueW(HKEY_CURRENT_USER, key, name, REG_SZ, value.c_str(),
                      static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t)));
  };
  write(L"Software\\Classes\\copypaste", nullptr, L"URL:CopyPaste Pairing");
  write(L"Software\\Classes\\copypaste", L"URL Protocol", L"");
  write(L"Software\\Classes\\copypaste\\DefaultIcon", nullptr, icon);
  write(L"Software\\Classes\\copypaste\\shell\\open\\command", nullptr,
        command);
}

}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  RegisterPairingProtocol();
  std::string pending_pairing_uri;
  const auto pairing_argument = std::find_if(
      command_line_arguments.begin(), command_line_arguments.end(),
      [](const std::string& argument) {
        return argument.rfind(kPairingUriPrefix, 0) == 0;
      });
  if (pairing_argument != command_line_arguments.end()) {
    pending_pairing_uri = *pairing_argument;
    command_line_arguments.erase(pairing_argument);
  }

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project, false, "", "", pending_pairing_uri, false);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1100, 760);
  if (!window.Create(L"CopyPaste", origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return EXIT_SUCCESS;
}
