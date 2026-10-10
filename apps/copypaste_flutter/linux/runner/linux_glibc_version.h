#ifndef FLUTTER_LINUX_GLIBC_VERSION_H_
#define FLUTTER_LINUX_GLIBC_VERSION_H_

#include <cstdint>
#include <optional>
#include <string>

// The runtime ABI floor for Linux module packages is GNU libc, not the kernel.
struct LinuxGlibcVersion {
  uint32_t major;
  uint32_t minor;
  uint32_t patch;

  std::string ToString() const;
};

// Returns the GNU libc version that loads this process. An unparseable value is
// unavailable rather than a substitute kernel or distribution version.
std::optional<LinuxGlibcVersion> linux_glibc_version();

#endif  // FLUTTER_LINUX_GLIBC_VERSION_H_
