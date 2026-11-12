#ifndef RUNNER_DEVICE_AUTHENTICATION_H_
#define RUNNER_DEVICE_AUTHENTICATION_H_

#include <windows.h>

#include <flutter/binary_messenger.h>
#include <flutter/encodable_value.h>
#include <flutter/method_channel.h>

#include <map>
#include <memory>
#include <string>

// Channel results for Windows Hello enumeration values. Negative inputs are
// the cancelled (-1) and failed (-2) asynchronous outcomes.
namespace DeviceAuthenticationResults {
const char* Availability(int availability);
const char* Verification(int verification);
}  // namespace DeviceAuthenticationResults

// Verifies the signed-in user with Windows Hello for one engine. Completions
// arrive on WinRT threads and are marshalled back through a message window.
class DeviceAuthentication {
 public:
  DeviceAuthentication(flutter::BinaryMessenger* messenger, HWND owner);
  ~DeviceAuthentication();
  DeviceAuthentication(const DeviceAuthentication&) = delete;
  DeviceAuthentication& operator=(const DeviceAuthentication&) = delete;

 private:
  struct Pending;
  using Result = std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>;

  static LRESULT CALLBACK SinkProc(HWND window, UINT message, WPARAM wparam,
                                   LPARAM lparam);
  bool EnsureSink();
  void Start(bool verify, const std::wstring& message, Result result);
  void Complete(UINT_PTR id, LPARAM code);

  HWND owner_;
  HWND sink_ = nullptr;
  UINT_PTR next_id_ = 0;
  std::map<UINT_PTR, std::unique_ptr<Pending>> pending_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

#endif  // RUNNER_DEVICE_AUTHENTICATION_H_
