#include "device_authentication.h"

#include <roapi.h>
#include <UserConsentVerifierInterop.h>
#include <windows.foundation.h>
#include <windows.security.credentials.ui.h>
#include <wrl/client.h>
#include <wrl/event.h>
#include <wrl/wrappers/corewrappers.h>

#include <flutter/standard_method_codec.h>

#include <utility>
#include <variant>

namespace {
using ABI::Windows::Foundation::AsyncStatus;
using ABI::Windows::Foundation::IAsyncInfo;
using ABI::Windows::Foundation::IAsyncOperation;
using ABI::Windows::Foundation::IAsyncOperationCompletedHandler;
using ABI::Windows::Security::Credentials::UI::IUserConsentVerifierStatics;
using ABI::Windows::Security::Credentials::UI::UserConsentVerificationResult;
using ABI::Windows::Security::Credentials::UI::UserConsentVerifierAvailability;
using Microsoft::WRL::Callback;
using Microsoft::WRL::ComPtr;
using Microsoft::WRL::Wrappers::HStringReference;

constexpr char kChannel[] = "com.copypaste.app/device_auth";
constexpr wchar_t kSinkClass[] = L"CopyPasteDeviceAuthenticationSink";
constexpr UINT kCompletionMessage = WM_APP + 0x41;
constexpr LPARAM kCancelledCode = -1;
constexpr LPARAM kFailedCode = -2;

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

const std::string* StringArgument(
    const flutter::MethodCall<flutter::EncodableValue>& call, const char* name) {
  const auto* arguments = std::get_if<flutter::EncodableMap>(call.arguments());
  if (arguments == nullptr) return nullptr;
  const auto iterator = arguments->find(flutter::EncodableValue(name));
  return iterator == arguments->end()
             ? nullptr
             : std::get_if<std::string>(&iterator->second);
}

// Posts the outcome to the sink; the handler never touches channel state.
template <typename T>
HRESULT Observe(const ComPtr<IAsyncOperation<T>>& operation, HWND sink,
                UINT_PTR id, ComPtr<IAsyncInfo>* info) {
  auto handler = Callback<IAsyncOperationCompletedHandler<T>>(
      [sink, id](IAsyncOperation<T>* completed, AsyncStatus status) -> HRESULT {
        LPARAM code = kFailedCode;
        T value{};
        if (status == AsyncStatus::Canceled) {
          code = kCancelledCode;
        } else if (status == AsyncStatus::Completed &&
                   SUCCEEDED(completed->GetResults(&value))) {
          code = static_cast<LPARAM>(value);
        }
        ::PostMessageW(sink, kCompletionMessage, id, code);
        return S_OK;
      });
  if (!handler) return E_OUTOFMEMORY;
  const HRESULT hr = operation.As(info);
  if (FAILED(hr)) return hr;
  return operation->put_Completed(handler.Get());
}

HRESULT RequestVerification(HWND owner, const std::wstring& message,
                            ComPtr<IAsyncOperation<UserConsentVerificationResult>>* operation) {
  const HStringReference text(message.c_str(), static_cast<unsigned int>(message.size()));
  ComPtr<IUserConsentVerifierInterop> interop;
  if (SUCCEEDED(::RoGetActivationFactory(
          HStringReference(RuntimeClass_Windows_Security_Credentials_UI_UserConsentVerifier).Get(),
          IID_PPV_ARGS(&interop)))) {
    return interop->RequestVerificationForWindowAsync(
        owner, text.Get(), IID_PPV_ARGS(operation->ReleaseAndGetAddressOf()));
  }
  // Builds without the window interop show the same prompt unparented.
  ComPtr<IUserConsentVerifierStatics> statics;
  const HRESULT hr = ::RoGetActivationFactory(
      HStringReference(RuntimeClass_Windows_Security_Credentials_UI_UserConsentVerifier).Get(),
      IID_PPV_ARGS(&statics));
  if (FAILED(hr)) return hr;
  return statics->RequestVerificationAsync(text.Get(), operation->ReleaseAndGetAddressOf());
}

HRESULT CheckAvailability(
    ComPtr<IAsyncOperation<UserConsentVerifierAvailability>>* operation) {
  ComPtr<IUserConsentVerifierStatics> statics;
  const HRESULT hr = ::RoGetActivationFactory(
      HStringReference(RuntimeClass_Windows_Security_Credentials_UI_UserConsentVerifier).Get(),
      IID_PPV_ARGS(&statics));
  if (FAILED(hr)) return hr;
  return statics->CheckAvailabilityAsync(operation->ReleaseAndGetAddressOf());
}
}  // namespace

namespace DeviceAuthenticationResults {
const char* Availability(int availability) {
  switch (availability) {
    case 0:  // Available
    case 4:  // DeviceBusy
      return "available";
    case 1:  // DeviceNotPresent: no PIN or biometric verifier is set up yet.
    case 2:  // NotConfiguredForUser
      return "notConfigured";
    default:  // DisabledByPolicy, failures
      return "unsupported";
  }
}

const char* Verification(int verification) {
  switch (verification) {
    case 0:  // Verified
      return "success";
    case 1:  // DeviceNotPresent
    case 2:  // NotConfiguredForUser
    case 3:  // DisabledByPolicy
    case -2:  // The request could not run.
      return "unavailable";
    case 5:  // RetriesExhausted
      return "lockedOut";
    default:  // Canceled, DeviceBusy, cancelled operation
      return "cancelled";
  }
}
}  // namespace DeviceAuthenticationResults

struct DeviceAuthentication::Pending {
  bool verify;
  Result result;
  ComPtr<IAsyncInfo> operation;
};

DeviceAuthentication::DeviceAuthentication(flutter::BinaryMessenger* messenger,
                                           HWND owner)
    : owner_(owner) {
  channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
      messenger, kChannel, &flutter::StandardMethodCodec::GetInstance());
  channel_->SetMethodCallHandler([this](const auto& call, auto result) {
    if (call.method_name() == "availability") {
      Start(false, {}, std::move(result));
    } else if (call.method_name() == "authenticate") {
      const auto* reason = StringArgument(call, "reason");
      const std::wstring message = reason == nullptr ? std::wstring() : Utf16FromUtf8(*reason);
      if (message.empty()) {
        result->Error("invalid_arguments");
        return;
      }
      for (const auto& entry : pending_) {
        if (entry.second->verify) {
          result->Error("busy");
          return;
        }
      }
      Start(true, message, std::move(result));
    } else {
      result->NotImplemented();
    }
  });
}

DeviceAuthentication::~DeviceAuthentication() {
  channel_->SetMethodCallHandler(nullptr);
  // The engine is going away with this object, so pending calls get no reply.
  for (auto& entry : pending_) {
    if (entry.second->operation) entry.second->operation->Cancel();
  }
  pending_.clear();
  if (sink_ != nullptr) ::DestroyWindow(sink_);
}

bool DeviceAuthentication::EnsureSink() {
  if (sink_ != nullptr) return true;
  const HINSTANCE instance = ::GetModuleHandleW(nullptr);
  WNDCLASSW window_class{};
  if (!::GetClassInfoW(instance, kSinkClass, &window_class)) {
    window_class.lpfnWndProc = SinkProc;
    window_class.hInstance = instance;
    window_class.lpszClassName = kSinkClass;
    if (::RegisterClassW(&window_class) == 0) return false;
  }
  sink_ = ::CreateWindowExW(0, kSinkClass, L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                            nullptr, instance, this);
  return sink_ != nullptr;
}

void DeviceAuthentication::Start(bool verify, const std::wstring& message,
                                 Result result) {
  const char* failure = verify ? DeviceAuthenticationResults::Verification(-2)
                               : DeviceAuthenticationResults::Availability(-2);
  if (!EnsureSink()) {
    result->Success(flutter::EncodableValue(std::string(failure)));
    return;
  }
  const UINT_PTR id = ++next_id_;
  auto pending = std::make_unique<Pending>();
  pending->verify = verify;
  HRESULT hr;
  if (verify) {
    ComPtr<IAsyncOperation<UserConsentVerificationResult>> operation;
    hr = RequestVerification(owner_, message, &operation);
    if (SUCCEEDED(hr)) hr = Observe(operation, sink_, id, &pending->operation);
  } else {
    ComPtr<IAsyncOperation<UserConsentVerifierAvailability>> operation;
    hr = CheckAvailability(&operation);
    if (SUCCEEDED(hr)) hr = Observe(operation, sink_, id, &pending->operation);
  }
  if (FAILED(hr)) {
    if (pending->operation) pending->operation->Cancel();
    result->Success(flutter::EncodableValue(std::string(failure)));
    return;
  }
  pending->result = std::move(result);
  pending_.emplace(id, std::move(pending));
}

void DeviceAuthentication::Complete(UINT_PTR id, LPARAM code) {
  const auto found = pending_.find(id);
  if (found == pending_.end()) return;
  auto pending = std::move(found->second);
  pending_.erase(found);
  const int value = static_cast<int>(code);
  pending->result->Success(flutter::EncodableValue(std::string(
      pending->verify ? DeviceAuthenticationResults::Verification(value)
                      : DeviceAuthenticationResults::Availability(value))));
}

LRESULT CALLBACK DeviceAuthentication::SinkProc(HWND window, UINT message,
                                                WPARAM wparam, LPARAM lparam) {
  if (message == WM_NCCREATE) {
    const auto* create = reinterpret_cast<const CREATESTRUCTW*>(lparam);
    ::SetWindowLongPtrW(window, GWLP_USERDATA,
                        reinterpret_cast<LONG_PTR>(create->lpCreateParams));
  } else if (message == kCompletionMessage) {
    auto* self = reinterpret_cast<DeviceAuthentication*>(
        ::GetWindowLongPtrW(window, GWLP_USERDATA));
    if (self != nullptr) self->Complete(static_cast<UINT_PTR>(wparam), lparam);
    return 0;
  }
  return ::DefWindowProcW(window, message, wparam, lparam);
}
