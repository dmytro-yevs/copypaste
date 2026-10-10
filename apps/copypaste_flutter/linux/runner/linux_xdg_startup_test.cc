// Run scripts/test-linux-xdg-startup.sh. The fixture only uses a private XDG
// root and never opens a URI or modifies the user's desktop preferences.

#include "linux_xdg_startup.h"

#include <gio/gio.h>
#include <glib.h>
#include <glib/gstdio.h>

#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#include <string>
#include <type_traits>
#include <utility>

// Xlib defines Status as a macro after the runner includes this adapter.
// This compile-only assertion preserves that include order in the fixture.
#define Status int
static_assert(std::is_same_v<
              decltype(std::declval<LinuxXdgStartup>().GetStatus()),
              LinuxXdgStartupStatus>);
#undef Status

namespace {

class XdgFixture {
 public:
  XdgFixture() {
    g_autoptr(GError) error = nullptr;
    root_ = g_dir_make_tmp("copypaste-xdg-startup.XXXXXX", &error);
    g_assert_no_error(error);
    g_assert_false(root_.empty());
    config_ = root_ + "/config";
    data_ = root_ + "/data";
    executable_ = root_ + "/Copy Paste % handler";
    const int executable = open(executable_.c_str(), O_WRONLY | O_CREAT | O_EXCL,
                                0700);
    g_assert_cmpint(executable, >=, 0);
    g_assert_cmpint(close(executable), ==, 0);
    g_setenv("XDG_CONFIG_HOME", config_.c_str(), TRUE);
    g_setenv("XDG_DATA_HOME", data_.c_str(), TRUE);
  }

  ~XdgFixture() { remove_tree(root_); }

  LinuxXdgStartup startup() const {
    g_autoptr(GError) error = nullptr;
    const auto startup =
        LinuxXdgStartup::CreateForTesting(executable_, config_, data_, &error);
    g_assert_no_error(error);
    g_assert_true(startup.has_value());
    return *startup;
  }

  const std::string& root() const { return root_; }
  const std::string& config() const { return config_; }
  const std::string& data() const { return data_; }
  const std::string& executable() const { return executable_; }

  void clear_entries() const {
    remove_tree(config_);
    remove_tree(data_);
  }

 private:
  static void remove_tree(const std::string& path) {
    GDir* directory = g_dir_open(path.c_str(), 0, nullptr);
    if (directory == nullptr) {
      g_remove(path.c_str());
      return;
    }
    const gchar* name = nullptr;
    while ((name = g_dir_read_name(directory)) != nullptr) {
      const std::string child = path + "/" + name;
      remove_tree(child);
    }
    g_dir_close(directory);
    g_rmdir(path.c_str());
  }

  std::string root_;
  std::string config_;
  std::string data_;
  std::string executable_;
};

XdgFixture* fixture = nullptr;

void test_exec_escaping() {
  const std::string entry = LinuxXdgStartup::DesktopEntryForExecutable(
      "/tmp/Copy Paste\"quote\\slash%code", false);
  g_assert_nonnull(strstr(
      entry.c_str(),
      "Exec=\"/tmp/Copy Paste\\\"quote\\\\slash%%code\" %U\n"));
  g_assert_nonnull(strstr(entry.c_str(), "MimeType=x-scheme-handler/copypaste;\n"));
}

void test_autostart_is_owned_and_atomic() {
  fixture->clear_entries();
  const LinuxXdgStartup startup = fixture->startup();
  g_assert_false(startup.GetStatus().start_at_login);

  g_autoptr(GError) error = nullptr;
  g_assert_true(startup.SetStartAtLogin(true, &error));
  g_assert_no_error(error);
  const std::string path = fixture->config() +
      "/autostart/com.copypaste.CopyPaste.desktop";
  gchar* contents = nullptr;
  gsize length = 0;
  g_assert_true(g_file_get_contents(path.c_str(), &contents, &length, &error));
  g_assert_no_error(error);
  const std::string expected =
      LinuxXdgStartup::DesktopEntryForExecutable(fixture->executable(), true);
  g_assert_cmpstr(contents, ==, expected.c_str());
  g_free(contents);
  g_assert_true(startup.GetStatus().start_at_login);

  g_assert_true(startup.SetStartAtLogin(false, &error));
  g_assert_no_error(error);
  g_assert_false(g_file_test(path.c_str(), G_FILE_TEST_EXISTS));

  const std::string user_entry =
      "[Desktop Entry]\n"
      "Name=User CopyPaste\n"
      "Comment=X-CopyPaste-Managed=true\n"
      "X-User-Note=X-CopyPaste-Managed=true\n"
      "Exec=/usr/bin/user-command %U\n";
  g_assert_true(g_file_set_contents(path.c_str(), user_entry.c_str(), -1, &error));
  g_assert_no_error(error);
  g_assert_false(startup.SetStartAtLogin(false, &error));
  g_assert_error(error, G_FILE_ERROR, G_FILE_ERROR_ACCES);
  g_clear_error(&error);
  g_assert_false(startup.SetStartAtLogin(true, &error));
  g_assert_error(error, G_FILE_ERROR, G_FILE_ERROR_ACCES);
  g_clear_error(&error);
  g_assert_true(g_file_test(path.c_str(), G_FILE_TEST_EXISTS));
  gchar* user_contents = nullptr;
  gsize user_length = 0;
  g_assert_true(
      g_file_get_contents(path.c_str(), &user_contents, &user_length, &error));
  g_assert_no_error(error);
  g_assert_cmpstr(user_contents, ==, user_entry.c_str());
  g_free(user_contents);

  const std::string old_appimage = "/home/test/CopyPaste-old.AppImage";
  const std::string old_entry =
      LinuxXdgStartup::DesktopEntryForExecutable(old_appimage, true);
  g_assert_true(g_file_set_contents(path.c_str(), old_entry.c_str(), -1, &error));
  g_assert_no_error(error);
  g_assert_false(startup.GetStatus().start_at_login);
  g_assert_true(startup.SetStartAtLogin(true, &error));
  g_assert_no_error(error);
  gchar* updated_contents = nullptr;
  gsize updated_length = 0;
  g_assert_true(g_file_get_contents(path.c_str(), &updated_contents,
                                    &updated_length, &error));
  g_assert_no_error(error);
  const std::string current_entry =
      LinuxXdgStartup::DesktopEntryForExecutable(fixture->executable(), true);
  g_assert_cmpstr(updated_contents, ==, current_entry.c_str());
  g_free(updated_contents);
  g_assert_true(startup.GetStatus().start_at_login);
  g_assert_true(startup.SetStartAtLogin(false, &error));
  g_assert_no_error(error);
  g_assert_false(g_file_test(path.c_str(), G_FILE_TEST_EXISTS));
}

void test_uri_registration_uses_private_xdg_home() {
  fixture->clear_entries();
  const LinuxXdgStartup startup = fixture->startup();
  g_autoptr(GError) error = nullptr;
  g_assert_true(startup.RegisterCopypasteUri(&error));
  g_assert_no_error(error);
  const std::string path = fixture->data() +
      "/applications/com.copypaste.CopyPaste.desktop";
  g_assert_true(g_file_test(path.c_str(), G_FILE_TEST_IS_REGULAR));
  g_autoptr(GAppInfo) handler = g_app_info_get_default_for_type(
      "x-scheme-handler/copypaste", TRUE);
  g_assert_nonnull(handler);
  g_assert_cmpstr(g_app_info_get_id(handler), ==,
                  "com.copypaste.CopyPaste.desktop");
  g_assert_true(startup.GetStatus().uri_registered);
}

void test_rejects_invalid_paths_and_uses_outer_appimage() {
  fixture->clear_entries();
  g_autoptr(GError) error = nullptr;
  const auto relative = LinuxXdgStartup::CreateForTesting(
      "relative/copypaste", fixture->config(), fixture->data(), &error);
  g_assert_false(relative.has_value());
  g_assert_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL);
  g_clear_error(&error);

  const auto missing = LinuxXdgStartup::CreateForTesting(
      fixture->root() + "/missing", fixture->config(), fixture->data(), &error);
  g_assert_false(missing.has_value());
  g_assert_nonnull(error);
  g_clear_error(&error);

  const std::string line_break_path = fixture->root() + "/line\nbreak";
  const int line_break_file = open(line_break_path.c_str(),
                                   O_WRONLY | O_CREAT | O_EXCL, 0700);
  g_assert_cmpint(line_break_file, >=, 0);
  g_assert_cmpint(close(line_break_file), ==, 0);
  const auto line_break = LinuxXdgStartup::CreateForTesting(
      line_break_path, fixture->config(), fixture->data(), &error);
  g_assert_false(line_break.has_value());
  g_assert_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL);
  g_clear_error(&error);

  g_setenv("APPIMAGE", fixture->executable().c_str(), TRUE);
  const auto appimage = LinuxXdgStartup::CreateForCurrentExecutable(&error);
  g_assert_no_error(error);
  g_assert_true(appimage.has_value());
  g_assert_true(appimage->SetStartAtLogin(true, &error));
  g_assert_no_error(error);
  g_unsetenv("APPIMAGE");
}

}  // namespace

int main(int argc, char** argv) {
  XdgFixture test_fixture;
  fixture = &test_fixture;
  g_test_init(&argc, &argv, nullptr);
  g_test_add_func("/linux/xdg_startup/exec_escaping", test_exec_escaping);
  g_test_add_func("/linux/xdg_startup/autostart_owned_atomic",
                  test_autostart_is_owned_and_atomic);
  g_test_add_func("/linux/xdg_startup/uri_private_xdg",
                  test_uri_registration_uses_private_xdg_home);
  g_test_add_func("/linux/xdg_startup/invalid_paths_and_appimage",
                  test_rejects_invalid_paths_and_uses_outer_appimage);
  return g_test_run();
}
