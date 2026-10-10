#include "linux_glibc_version.h"

#include <charconv>
#include <gnu/libc-version.h>
#include <string_view>

namespace {

std::optional<uint32_t> parse_component(std::string_view value) {
  if (value.empty()) return std::nullopt;
  uint32_t component = 0;
  const auto [end, error] = std::from_chars(
      value.data(), value.data() + value.size(), component);
  if (error != std::errc() || end != value.data() + value.size()) {
    return std::nullopt;
  }
  return component;
}

std::optional<LinuxGlibcVersion> parse_glibc_version(const char* value) {
  if (value == nullptr || *value == '\0') return std::nullopt;
  const std::string_view raw(value);
  const size_t first_dot = raw.find('.');
  if (first_dot == std::string_view::npos) return std::nullopt;
  const size_t second_dot = raw.find('.', first_dot + 1);
  const std::string_view major = raw.substr(0, first_dot);
  const std::string_view minor = raw.substr(
      first_dot + 1, second_dot == std::string_view::npos
          ? std::string_view::npos : second_dot - first_dot - 1);
  const std::string_view patch = second_dot == std::string_view::npos
      ? std::string_view("0") : raw.substr(second_dot + 1);
  if (second_dot != std::string_view::npos &&
      raw.find('.', second_dot + 1) != std::string_view::npos) {
    return std::nullopt;
  }
  const auto parsed_major = parse_component(major);
  const auto parsed_minor = parse_component(minor);
  const auto parsed_patch = parse_component(patch);
  if (!parsed_major || !parsed_minor || !parsed_patch) return std::nullopt;
  return LinuxGlibcVersion{*parsed_major, *parsed_minor, *parsed_patch};
}

}  // namespace

std::string LinuxGlibcVersion::ToString() const {
  return std::to_string(major) + "." + std::to_string(minor) + "." +
      std::to_string(patch);
}

std::optional<LinuxGlibcVersion> linux_glibc_version() {
  return parse_glibc_version(gnu_get_libc_version());
}
