#include "window_material.h"

#include <dwmapi.h>
#include <flutter/standard_method_codec.h>

#include <optional>
#include <string>

#include "win32_window.h"

namespace {

constexpr char kWindowMaterialChannel[] = "com.copypaste.app/window_material";
constexpr char kSystemAccentChannel[] = "com.copypaste.app/system_accent";

// Settings > Personalization > Colors > Accent color, as opaque ARGB. DWM
// stores it as 0xAABBGGRR.
std::optional<int64_t> SystemAccent() {
  DWORD value = 0;
  DWORD size = sizeof(value);
  if (::RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\DWM",
                     L"AccentColor", RRF_RT_REG_DWORD, nullptr, &value,
                     &size) != ERROR_SUCCESS) {
    return std::nullopt;
  }
  const int64_t red = value & 0xFF;
  const int64_t green = (value >> 8) & 0xFF;
  const int64_t blue = (value >> 16) & 0xFF;
  return 0xFF000000LL | (red << 16) | (green << 8) | blue;
}

flutter::EncodableValue AccentValue(const std::optional<int64_t>& accent) {
  return accent ? flutter::EncodableValue(*accent) : flutter::EncodableValue();
}

// DWMWA_SYSTEMBACKDROP_TYPE and its DWM_SYSTEMBACKDROP_TYPE values from the
// Windows 11 22H2 SDK; named here so older SDKs still compile.
constexpr DWORD kSystemBackdropAttribute = 38;
constexpr int kBackdropAuto = 0;
constexpr int kBackdropNone = 1;
constexpr int kBackdropMica = 2;
constexpr int kBackdropAcrylic = 3;

bool SetBackdrop(HWND window, int type) {
  return SUCCEEDED(::DwmSetWindowAttribute(window, kSystemBackdropAttribute,
                                           &type, sizeof(type)));
}

void ExtendFrame(HWND window, bool extend) {
  const int inset = extend ? -1 : 0;
  const MARGINS margins{inset, inset, inset, inset};
  ::DwmExtendFrameIntoClientArea(window, &margins);
}

// Settings > Personalization > Colors > Transparency effects.
bool TransparencyEffectsEnabled() {
  DWORD value = 1;
  DWORD size = sizeof(value);
  const LSTATUS status = ::RegGetValueW(
      HKEY_CURRENT_USER,
      L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
      L"EnableTransparency", RRF_RT_REG_DWORD, nullptr, &value, &size);
  return status != ERROR_SUCCESS || value != 0;
}

bool HighContrastOn() {
  HIGHCONTRASTW contrast{};
  contrast.cbSize = sizeof(contrast);
  return ::SystemParametersInfoW(SPI_GETHIGHCONTRAST, sizeof(contrast),
                                 &contrast, 0) &&
         (contrast.dwFlags & HCF_HIGHCONTRASTON) != 0;
}

const std::string* ModeArgument(const flutter::EncodableValue* arguments) {
  const auto* map = arguments == nullptr
                        ? nullptr
                        : std::get_if<flutter::EncodableMap>(arguments);
  if (map == nullptr) return nullptr;
  const auto entry = map->find(flutter::EncodableValue("mode"));
  return entry == map->end() ? nullptr
                             : std::get_if<std::string>(&entry->second);
}

const bool* EnabledArgument(const flutter::EncodableValue* arguments) {
  const auto* map = arguments == nullptr
                        ? nullptr
                        : std::get_if<flutter::EncodableMap>(arguments);
  if (map == nullptr) return nullptr;
  const auto entry = map->find(flutter::EncodableValue("enabled"));
  return entry == map->end() ? nullptr : std::get_if<bool>(&entry->second);
}

}  // namespace

WindowMaterial::WindowMaterial(flutter::BinaryMessenger* messenger,
                               HWND window, Role role)
    : window_(window),
      role_(role),
      // The automatic backdrop is the default, so probing with it changes
      // nothing where the attribute exists.
      supported_(SetBackdrop(window, kBackdropAuto)),
      accent_(SystemAccent()),
      channel_(std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          messenger, kWindowMaterialChannel,
          &flutter::StandardMethodCodec::GetInstance())),
      accent_channel_(
          std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
              messenger, kSystemAccentChannel,
              &flutter::StandardMethodCodec::GetInstance())) {
  accent_channel_->SetMethodCallHandler([this](const auto& call, auto result) {
    if (call.method_name() != "read") {
      result->NotImplemented();
      return;
    }
    accent_ = SystemAccent();
    result->Success(AccentValue(accent_));
  });
  channel_->SetMethodCallHandler([this](const auto& call, auto result) {
    if (call.method_name() == "setAppearance") {
      const std::string* mode = ModeArgument(call.arguments());
      if (mode == nullptr) {
        result->Error("invalid_arguments");
        return;
      }
      Win32Window::SetDarkModeOverride(
          *mode == "dark"    ? std::optional<bool>(true)
          : *mode == "light" ? std::optional<bool>(false)
                             : std::nullopt);
      Win32Window::UpdateTheme(window_);
      result->Success();
      return;
    }
    if (call.method_name() != "setEnabled") {
      result->NotImplemented();
      return;
    }
    const bool* enabled = EnabledArgument(call.arguments());
    if (enabled == nullptr) {
      result->Error("invalid_arguments");
      return;
    }
    enabled_ = *enabled;
    result->Success(Apply());
  });
}

WindowMaterial::~WindowMaterial() {
  channel_->SetMethodCallHandler(nullptr);
  accent_channel_->SetMethodCallHandler(nullptr);
}

void WindowMaterial::HandleSettingChange() {
  const auto accent = SystemAccent();
  if (accent != accent_) {
    accent_ = accent;
    accent_channel_->InvokeMethod(
        "changed", std::make_unique<flutter::EncodableValue>(AccentValue(accent)));
  }
  const bool was_active = active_;
  const bool was_reduced = reduced_;
  auto state = Apply();
  if (active_ == was_active && reduced_ == was_reduced) return;
  channel_->InvokeMethod("stateChanged",
                         std::make_unique<flutter::EncodableValue>(state));
}

flutter::EncodableValue WindowMaterial::Apply() {
  reduced_ = !TransparencyEffectsEnabled() || HighContrastOn();
  const bool wanted = supported_ && enabled_ && !reduced_;
  if (wanted != active_) {
    if (wanted) {
      ExtendFrame(window_, true);
      active_ = SetBackdrop(window_, role_ == Role::kMain ? kBackdropMica
                                                          : kBackdropAcrylic);
      if (!active_) ExtendFrame(window_, false);
    } else {
      SetBackdrop(window_, kBackdropNone);
      ExtendFrame(window_, false);
      active_ = false;
    }
  }
  const std::string material =
      !active_ ? "none" : role_ == Role::kMain ? "mica" : "acrylic";
  return flutter::EncodableValue(flutter::EncodableMap{
      {flutter::EncodableValue("material"), flutter::EncodableValue(material)},
      {flutter::EncodableValue("supported"),
       flutter::EncodableValue(supported_)},
      {flutter::EncodableValue("reducedTransparency"),
       flutter::EncodableValue(reduced_)},
  });
}
