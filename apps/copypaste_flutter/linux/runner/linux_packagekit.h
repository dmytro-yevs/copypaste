#ifndef RUNNER_LINUX_PACKAGEKIT_H_
#define RUNNER_LINUX_PACKAGEKIT_H_

#include <functional>
#include <string>

enum class LinuxPackageKind { kUnavailable, kDeb, kRpm };

struct LinuxPackageInstallation {
  LinuxPackageKind kind = LinuxPackageKind::kUnavailable;
  std::string architecture;
  std::string reason;

  bool available() const { return kind != LinuxPackageKind::kUnavailable; }
};

enum class LinuxPackageInstallResult {
  kRestartRequired,
  kPermissionRequired,
  kFailed,
};

// Uses the system PackageKit service for packages that declare immutable,
// root-owned package provenance. The completion callback fires only after the
// PackageKit transaction has reached a terminal state.
class LinuxPackageKit {
 public:
  LinuxPackageInstallation Detect() const;

  void Install(const std::string& package_path,
               const std::string& expected_sha256,
               std::function<void(LinuxPackageInstallResult, std::string)> done);
};

#endif  // RUNNER_LINUX_PACKAGEKIT_H_
