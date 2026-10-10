#include "linux_xdg_startup.h"

#include <gio/gdesktopappinfo.h>
#include <gio/gio.h>
#include <glib/gstdio.h>

#include <cerrno>
#include <climits>
#include <cstring>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#include <string>
#include <utility>

namespace {

constexpr char kDesktopFileName[] = "com.copypaste.CopyPaste.desktop";
constexpr char kPackagedExecutable[] = "/usr/lib/copypaste/copypaste";
constexpr char kManagedKey[] = "X-CopyPaste-Managed=true";

bool is_regular_executable_owned_by(const std::string& path, uid_t owner) {
  struct stat metadata = {};
  return !path.empty() && path.front() == '/' &&
      stat(path.c_str(), &metadata) == 0 && S_ISREG(metadata.st_mode) &&
      metadata.st_uid == owner && access(path.c_str(), X_OK) == 0;
}

std::optional<std::string> canonical_executable(const std::string& path,
                                                uid_t owner, GError** error) {
  if (path.empty() || path.front() != '/') {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL,
                "The CopyPaste executable must be an absolute path.");
    return std::nullopt;
  }
  char resolved[PATH_MAX] = {};
  if (realpath(path.c_str(), resolved) == nullptr ||
      !is_regular_executable_owned_by(resolved, owner)) {
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(errno),
                "The CopyPaste executable is unavailable.");
    return std::nullopt;
  }
  if (std::strpbrk(resolved, "\n\r") != nullptr) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL,
                "The CopyPaste executable path cannot contain a line break.");
    return std::nullopt;
  }
  return std::string(resolved);
}

std::string quote_desktop_exec_argument(const std::string& value) {
  std::string quoted{"\""};
  for (const char character : value) {
    switch (character) {
      case '\\':
      case '"':
      case '`':
      case '$':
        quoted.push_back('\\');
        quoted.push_back(character);
        break;
      case '%':
        // Desktop Entry field codes use percent. A literal percent in a path
        // must therefore be escaped before GLib parses the Exec value.
        quoted.append("%%");
        break;
      default:
        quoted.push_back(character);
        break;
    }
  }
  quoted.push_back('"');
  return quoted;
}

bool write_all(int fd, const char* bytes, size_t length) {
  size_t written = 0;
  while (written < length) {
    const ssize_t result = write(fd, bytes + written, length - written);
    if (result > 0) {
      written += static_cast<size_t>(result);
      continue;
    }
    if (result < 0 && errno == EINTR) continue;
    return false;
  }
  return true;
}

bool atomic_write(const std::string& path, const std::string& contents,
                  GError** error) {
  g_autofree gchar* directory = g_path_get_dirname(path.c_str());
  if (g_mkdir_with_parents(directory, 0700) != 0) {
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(errno),
                "Unable to create the CopyPaste XDG directory: %s",
                g_strerror(errno));
    return false;
  }
  g_autofree gchar* temporary = g_build_filename(
      directory, ".com.copypaste.CopyPaste.desktop.XXXXXX", nullptr);
  const int fd = g_mkstemp_full(temporary, O_WRONLY | O_CLOEXEC, 0600);
  if (fd < 0) {
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(errno),
                "Unable to create the CopyPaste XDG entry: %s", g_strerror(errno));
    return false;
  }
  bool written = write_all(fd, contents.data(), contents.size());
  if (written && fsync(fd) != 0) written = false;
  if (close(fd) != 0) written = false;
  if (!written) {
    const int saved_errno = errno;
    unlink(temporary);
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(saved_errno),
                "Unable to write the CopyPaste XDG entry: %s",
                g_strerror(saved_errno));
    return false;
  }
  if (g_rename(temporary, path.c_str()) != 0) {
    const int saved_errno = errno;
    unlink(temporary);
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(saved_errno),
                "Unable to install the CopyPaste XDG entry: %s",
                g_strerror(saved_errno));
    return false;
  }
  return true;
}

bool current_user_regular_file(const std::string& path) {
  struct stat metadata = {};
  return lstat(path.c_str(), &metadata) == 0 && S_ISREG(metadata.st_mode) &&
      metadata.st_uid == getuid();
}

bool has_managed_desktop_key(const gchar* contents, gsize length) {
  bool desktop_entry = false;
  bool managed = false;
  const gchar* line = contents;
  const gchar* end = contents + length;
  while (line < end) {
    const gchar* next = static_cast<const gchar*>(
        memchr(line, '\n', static_cast<size_t>(end - line)));
    const gsize line_length = next == nullptr
        ? static_cast<gsize>(end - line)
        : static_cast<gsize>(next - line);
    const std::string value(line, line_length);
    if (value == "[Desktop Entry]") {
      desktop_entry = true;
    } else if (desktop_entry && !value.empty() && value.front() == '[') {
      desktop_entry = false;
    } else if (desktop_entry && value == kManagedKey) {
      managed = true;
    }
    line = next == nullptr ? end : next + 1;
  }
  return managed;
}

}  // namespace

LinuxXdgStartup::LinuxXdgStartup(std::string executable,
                                 std::string config_home,
                                 std::string data_home)
    : executable_(std::move(executable)),
      config_home_(std::move(config_home)),
      data_home_(std::move(data_home)) {}

std::optional<LinuxXdgStartup> LinuxXdgStartup::CreateForCurrentExecutable(
    GError** error) {
  const gchar* appimage = g_getenv("APPIMAGE");
  if (appimage != nullptr && *appimage != '\0') {
    const auto executable = canonical_executable(appimage, getuid(), error);
    if (!executable) return std::nullopt;
    return LinuxXdgStartup(*executable, g_get_user_config_dir(),
                           g_get_user_data_dir());
  }

  char target[PATH_MAX] = {};
  const ssize_t size = readlink("/proc/self/exe", target, sizeof(target) - 1);
  if (size <= 0 || size >= static_cast<ssize_t>(sizeof(target) - 1)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_NOENT,
                "The current CopyPaste executable is unavailable.");
    return std::nullopt;
  }
  target[size] = '\0';
  const auto executable = canonical_executable(
      target, g_strcmp0(target, kPackagedExecutable) == 0 ? 0 : getuid(), error);
  if (!executable) return std::nullopt;
  if (*executable == kPackagedExecutable &&
      !is_regular_executable_owned_by(*executable, 0)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_ACCES,
                "The packaged CopyPaste executable is unavailable.");
    return std::nullopt;
  }
  return LinuxXdgStartup(*executable, g_get_user_config_dir(),
                         g_get_user_data_dir());
}

std::optional<LinuxXdgStartup> LinuxXdgStartup::CreateForTesting(
    const std::string& executable, const std::string& config_home,
    const std::string& data_home, GError** error) {
  const auto canonical = canonical_executable(executable, getuid(), error);
  if (!canonical) return std::nullopt;
  if (config_home.empty() || config_home.front() != '/' || data_home.empty() ||
      data_home.front() != '/') {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL,
                "The XDG homes must be absolute paths.");
    return std::nullopt;
  }
  return LinuxXdgStartup(*canonical, config_home, data_home);
}

std::string LinuxXdgStartup::autostart_path() const {
  return config_home_ + "/autostart/" + kDesktopFileName;
}

std::string LinuxXdgStartup::desktop_entry_path() const {
  return data_home_ + "/applications/" + kDesktopFileName;
}

std::string LinuxXdgStartup::DesktopEntryForExecutable(
    const std::string& executable, bool autostart) {
  std::string entry =
      "[Desktop Entry]\n"
      "Type=Application\n"
      "Name=CopyPaste\n"
      "Comment=Encrypted clipboard history\n"
      "Exec=" + quote_desktop_exec_argument(executable) + " %U\n"
      "Icon=com.copypaste.CopyPaste\n"
      "Terminal=false\n"
      "Categories=Utility;\n"
      "StartupNotify=true\n"
      "MimeType=x-scheme-handler/copypaste;\n";
  if (autostart) entry.append("X-GNOME-Autostart-enabled=true\n");
  entry.append(kManagedKey).append("\n");
  return entry;
}

bool LinuxXdgStartup::owns_entry_for_current_executable(
    const std::string& path, bool autostart) const {
  if (!current_user_regular_file(path)) return false;
  gchar* contents = nullptr;
  gsize length = 0;
  if (!g_file_get_contents(path.c_str(), &contents, &length, nullptr)) return false;
  const std::string expected = DesktopEntryForExecutable(executable_, autostart);
  const bool owned = has_managed_desktop_key(contents, length) &&
      expected.size() == length &&
      std::memcmp(contents, expected.data(), length) == 0;
  g_free(contents);
  return owned;
}

bool LinuxXdgStartup::write_owned_entry(const std::string& path, bool autostart,
                                         GError** error) const {
  if (g_file_test(path.c_str(), G_FILE_TEST_EXISTS) &&
      !owns_entry_for_current_executable(path, autostart)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_ACCES,
                "Refusing to replace an XDG entry not owned by CopyPaste.");
    return false;
  }
  return atomic_write(path, DesktopEntryForExecutable(executable_, autostart), error);
}

LinuxXdgStartupStatus LinuxXdgStartup::Status() const {
  LinuxXdgStartupStatus status;
  status.start_at_login =
      owns_entry_for_current_executable(autostart_path(), true);
  if (!owns_entry_for_current_executable(desktop_entry_path(), false)) return status;
  g_autoptr(GAppInfo) registered =
      g_app_info_get_default_for_uri_scheme("copypaste");
  status.uri_registered = registered != nullptr &&
      g_strcmp0(g_app_info_get_id(registered), kDesktopFileName) == 0;
  return status;
}

bool LinuxXdgStartup::SetStartAtLogin(bool enabled, GError** error) const {
  const std::string path = autostart_path();
  if (enabled) return write_owned_entry(path, true, error);
  if (!g_file_test(path.c_str(), G_FILE_TEST_EXISTS)) return true;
  if (!owns_entry_for_current_executable(path, true)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_ACCES,
                "Refusing to remove an XDG entry not owned by CopyPaste.");
    return false;
  }
  if (g_remove(path.c_str()) != 0) {
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(errno),
                "Unable to remove the CopyPaste autostart entry: %s",
                g_strerror(errno));
    return false;
  }
  return true;
}

bool LinuxXdgStartup::RegisterCopypasteUri(GError** error) const {
  const std::string path = desktop_entry_path();
  if (!write_owned_entry(path, false, error)) return false;
  g_autoptr(GDesktopAppInfo) app_info =
      g_desktop_app_info_new_from_filename(path.c_str());
  if (app_info == nullptr) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL,
                "Unable to load the CopyPaste URI desktop entry.");
    return false;
  }
  if (!g_app_info_set_as_default_for_uri_scheme(
          G_APP_INFO(app_info), "copypaste", nullptr, error)) {
    return false;
  }
  return true;
}
