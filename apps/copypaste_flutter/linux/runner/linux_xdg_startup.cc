#include "linux_xdg_startup.h"

#include <gio/gio.h>
#include <glib/gstdio.h>

#if defined(__linux__)
#include <gio/gdesktopappinfo.h>
#endif

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
constexpr char kApplicationId[] = "com.copypaste.CopyPaste";
constexpr char kUriHandlerContentType[] = "x-scheme-handler/copypaste";

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

bool decode_desktop_exec_argument(const std::string& quoted,
                                  std::string* executable) {
  if (quoted.size() < 2 || quoted.front() != '"' || quoted.back() != '"') {
    return false;
  }
  std::string decoded;
  for (size_t index = 1; index + 1 < quoted.size(); ++index) {
    const char character = quoted[index];
    if (character == '\\') {
      if (++index + 1 >= quoted.size()) return false;
      const char escaped = quoted[index];
      if (escaped != '\\' && escaped != '"' && escaped != '`' &&
          escaped != '$') {
        return false;
      }
      decoded.push_back(escaped);
    } else if (character == '%') {
      if (++index + 1 >= quoted.size() || quoted[index] != '%') return false;
      decoded.push_back('%');
    } else if (character == '"' || character == '\n' || character == '\r') {
      return false;
    } else {
      decoded.push_back(character);
    }
  }
  if (!g_path_is_absolute(decoded.c_str()) || decoded.empty() ||
      std::strpbrk(decoded.c_str(), "\n\r") != nullptr) {
    return false;
  }
  *executable = std::move(decoded);
  return true;
}

bool is_plain_executable_path(const gchar* value, std::string* executable) {
  if (value == nullptr || !g_path_is_absolute(value) || *value == '\0' ||
      std::strpbrk(value, "\n\r") != nullptr) {
    return false;
  }
  *executable = value;
  return true;
}

bool key_matches(GKeyFile* entry, const char* key, const char* expected) {
  g_autofree gchar* value =
      g_key_file_get_string(entry, "Desktop Entry", key, nullptr);
  return value != nullptr && g_strcmp0(value, expected) == 0;
}

bool has_only_managed_keys(GKeyFile* entry, bool autostart) {
  static constexpr const char* kBaseKeys[] = {
      "Type", "Name", "Comment", "Exec", "TryExec", "Icon", "Terminal",
      "Categories", "StartupNotify", "MimeType", "X-CopyPaste-ApplicationId",
      "X-CopyPaste-Managed",
  };
  gsize count = 0;
  g_auto(GStrv) keys = g_key_file_get_keys(entry, "Desktop Entry", &count, nullptr);
  if (keys == nullptr || count != G_N_ELEMENTS(kBaseKeys) + (autostart ? 1 : 0)) {
    return false;
  }
  for (const char* key : kBaseKeys) {
    if (!g_key_file_has_key(entry, "Desktop Entry", key, nullptr)) return false;
  }
  return !autostart || g_key_file_has_key(
      entry, "Desktop Entry", "X-GNOME-Autostart-enabled", nullptr);
}

bool is_managed_desktop_entry(const gchar* contents, gsize length,
                              bool autostart) {
  g_autoptr(GKeyFile) entry = g_key_file_new();
  if (!g_key_file_load_from_data(entry, contents, length, G_KEY_FILE_NONE,
                                 nullptr)) {
    return false;
  }
  gsize group_count = 0;
  g_auto(GStrv) groups = g_key_file_get_groups(entry, &group_count);
  if (group_count != 1 || g_strcmp0(groups[0], "Desktop Entry") != 0 ||
      !has_only_managed_keys(entry, autostart) ||
      !key_matches(entry, "Type", "Application") ||
      !key_matches(entry, "Name", "CopyPaste") ||
      !key_matches(entry, "Comment", "Encrypted clipboard history") ||
      !key_matches(entry, "Icon", kApplicationId) ||
      !key_matches(entry, "Terminal", "false") ||
      !key_matches(entry, "Categories", "Utility;") ||
      !key_matches(entry, "StartupNotify", "true") ||
      !key_matches(entry, "MimeType", "x-scheme-handler/copypaste;") ||
      !key_matches(entry, "X-CopyPaste-ApplicationId", kApplicationId) ||
      !key_matches(entry, "X-CopyPaste-Managed", "true") ||
      (autostart && !key_matches(
          entry, "X-GNOME-Autostart-enabled", "true"))) {
    return false;
  }
  g_autofree gchar* exec =
      g_key_file_get_string(entry, "Desktop Entry", "Exec", nullptr);
  g_autofree gchar* try_exec =
      g_key_file_get_string(entry, "Desktop Entry", "TryExec", nullptr);
  if (exec == nullptr || try_exec == nullptr) return false;
  const std::string exec_value(exec);
  static constexpr char kUriFieldCode[] = " %U";
  if (exec_value.size() <= strlen(kUriFieldCode) ||
      exec_value.compare(exec_value.size() - strlen(kUriFieldCode),
                         strlen(kUriFieldCode), kUriFieldCode) != 0) {
    return false;
  }
  const std::string quoted_exec =
      exec_value.substr(0, exec_value.size() - strlen(kUriFieldCode));
  std::string executable;
  std::string try_executable;
  return decode_desktop_exec_argument(quoted_exec, &executable) &&
      is_plain_executable_path(try_exec, &try_executable) &&
      executable == try_executable;
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
  g_autoptr(GKeyFile) entry = g_key_file_new();
  g_key_file_set_string(entry, "Desktop Entry", "Type", "Application");
  g_key_file_set_string(entry, "Desktop Entry", "Name", "CopyPaste");
  g_key_file_set_string(entry, "Desktop Entry", "Comment",
                        "Encrypted clipboard history");
  const std::string exec = quote_desktop_exec_argument(executable) + " %U";
  // Exec needs Desktop Entry quoting first. GKeyFile then escapes that value
  // for its own file syntax, preserving quotes, backslashes, and percent text.
  g_key_file_set_string(entry, "Desktop Entry", "Exec", exec.c_str());
  g_key_file_set_string(entry, "Desktop Entry", "TryExec", executable.c_str());
  g_key_file_set_string(entry, "Desktop Entry", "Icon", kApplicationId);
  g_key_file_set_string(entry, "Desktop Entry", "Terminal", "false");
  g_key_file_set_string(entry, "Desktop Entry", "Categories", "Utility;");
  g_key_file_set_string(entry, "Desktop Entry", "StartupNotify", "true");
  const std::string mime_type = std::string(kUriHandlerContentType) + ";";
  g_key_file_set_string(entry, "Desktop Entry", "MimeType",
                        mime_type.c_str());
  g_key_file_set_string(entry, "Desktop Entry", "X-CopyPaste-ApplicationId",
                        kApplicationId);
  if (autostart) {
    g_key_file_set_string(entry, "Desktop Entry", "X-GNOME-Autostart-enabled",
                          "true");
  }
  g_key_file_set_string(entry, "Desktop Entry", "X-CopyPaste-Managed", "true");
  gsize length = 0;
  g_autofree gchar* contents = g_key_file_to_data(entry, &length, nullptr);
  return std::string(contents, length);
}

bool LinuxXdgStartup::owns_entry_for_current_executable(
    const std::string& path, bool autostart) const {
  if (!owns_managed_entry(path, autostart)) return false;
  gchar* contents = nullptr;
  gsize length = 0;
  if (!g_file_get_contents(path.c_str(), &contents, &length, nullptr)) return false;
  const std::string expected = DesktopEntryForExecutable(executable_, autostart);
  const bool matches = expected.size() == length &&
      std::memcmp(contents, expected.data(), length) == 0;
  g_free(contents);
  return matches;
}

bool LinuxXdgStartup::owns_managed_entry(const std::string& path,
                                         bool autostart) const {
  if (!current_user_regular_file(path)) return false;
  gchar* contents = nullptr;
  gsize length = 0;
  if (!g_file_get_contents(path.c_str(), &contents, &length, nullptr)) return false;
  const bool owned = is_managed_desktop_entry(contents, length, autostart);
  g_free(contents);
  return owned;
}

bool LinuxXdgStartup::write_owned_entry(const std::string& path, bool autostart,
                                         GError** error) const {
  if (g_file_test(path.c_str(), G_FILE_TEST_EXISTS) &&
      !owns_managed_entry(path, autostart)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_ACCES,
                "Refusing to replace an XDG entry not owned by CopyPaste.");
    return false;
  }
  return atomic_write(path, DesktopEntryForExecutable(executable_, autostart), error);
}

LinuxXdgStartupStatus LinuxXdgStartup::GetStatus() const {
  LinuxXdgStartupStatus status;
  status.start_at_login =
      owns_entry_for_current_executable(autostart_path(), true);
  if (!owns_entry_for_current_executable(desktop_entry_path(), false)) return status;
  g_autoptr(GAppInfo) registered =
      g_app_info_get_default_for_type(kUriHandlerContentType, TRUE);
  status.uri_registered = registered != nullptr &&
      g_strcmp0(g_app_info_get_id(registered), kDesktopFileName) == 0;
  return status;
}

bool LinuxXdgStartup::SetStartAtLogin(bool enabled, GError** error) const {
  const std::string path = autostart_path();
  if (enabled) return write_owned_entry(path, true, error);
  if (!g_file_test(path.c_str(), G_FILE_TEST_EXISTS)) return true;
  if (!owns_managed_entry(path, true)) {
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
#if !defined(__linux__)
  g_set_error(error, G_IO_ERROR, G_IO_ERROR_NOT_SUPPORTED,
              "CopyPaste URI registration requires Linux desktop integration.");
  return false;
#else
  g_autoptr(GDesktopAppInfo) app_info =
      g_desktop_app_info_new_from_filename(path.c_str());
  if (app_info == nullptr) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL,
                "Unable to load the CopyPaste URI desktop entry.");
    return false;
  }
  if (!g_app_info_set_as_default_for_type(
          G_APP_INFO(app_info), kUriHandlerContentType, error)) {
    return false;
  }
  return true;
#endif
}
