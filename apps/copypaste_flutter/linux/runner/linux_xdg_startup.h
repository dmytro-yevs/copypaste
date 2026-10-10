#ifndef FLUTTER_LINUX_XDG_STARTUP_H_
#define FLUTTER_LINUX_XDG_STARTUP_H_

#include <glib.h>

#include <optional>
#include <string>

// The user-scoped XDG files that CopyPaste owns for opt-in startup and URI
// handling. Package files remain immutable; this adapter only writes below the
// current user's XDG config and data homes.
struct LinuxXdgStartupStatus {
  bool start_at_login = false;
  bool uri_registered = false;
};

class LinuxXdgStartup {
 public:
  // Resolves the real outer AppImage when APPIMAGE is set. Otherwise the
  // packaged executable remains /usr/lib/copypaste/copypaste, while a
  // development executable must be an executable owned by the current user.
  static std::optional<LinuxXdgStartup> CreateForCurrentExecutable(
      GError** error);

  // Visible for the isolated native fixture. Production callers must use
  // CreateForCurrentExecutable so desktop entries always launch this process.
  static std::optional<LinuxXdgStartup> CreateForTesting(
      const std::string& executable, const std::string& config_home,
      const std::string& data_home, GError** error);

  LinuxXdgStartupStatus GetStatus() const;
  bool SetStartAtLogin(bool enabled, GError** error) const;
  bool RegisterCopypasteUri(GError** error) const;

  static std::string DesktopEntryForExecutable(const std::string& executable,
                                               bool autostart);

 private:
  LinuxXdgStartup(std::string executable, std::string config_home,
                  std::string data_home);

  std::string autostart_path() const;
  std::string desktop_entry_path() const;
  bool owns_managed_entry(const std::string& path, bool autostart) const;
  bool owns_entry_for_current_executable(const std::string& path,
                                         bool autostart) const;
  bool write_owned_entry(const std::string& path, bool autostart,
                         GError** error) const;

  std::string executable_;
  std::string config_home_;
  std::string data_home_;
};

#endif  // FLUTTER_LINUX_XDG_STARTUP_H_
