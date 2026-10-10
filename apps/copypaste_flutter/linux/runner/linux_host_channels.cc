#include "linux_host_channels.h"
#include "linux_gnome_shortcuts.h"
#include "linux_glibc_version.h"
#include "linux_packagekit.h"
#include "linux_portal.h"
#include "linux_quick_paste_window.h"
#include "linux_restart_helper.h"
#include "linux_release_page.h"
#include "linux_xdg_startup.h"
#include "linux_x11_quick_paste.h"

#include <unistd.h>

#include <glib/gstdio.h>
#include <gio/gio.h>
#include <gdk/gdk.h>
#if defined(GDK_WINDOWING_WAYLAND)
#include <gdk/gdkwayland.h>
#endif
#if defined(GDK_WINDOWING_X11)
#include <gdk/gdkx.h>
#endif

#include <array>
#include <cstring>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <sys/stat.h>
#include <vector>
#include <memory>
#include <string>

namespace {

constexpr char kSecurity[] = "com.copypaste.app/security";
constexpr char kPairing[] = "com.copypaste.app/pairing_presentation_host";
constexpr char kQuickPasteHost[] = "com.copypaste.app/quick_paste_host";
constexpr char kQuickPasteContext[] = "com.copypaste.app/quick_paste_context";
constexpr char kShortcuts[] = "com.copypaste.app/linux_shortcuts";
constexpr char kIntegration[] = "com.copypaste.app/linux_integration";
constexpr char kUpdates[] = "com.copypaste.app/app_update";
constexpr char kLifecycle[] = "com.copypaste.app/lifecycle";
constexpr char kPairingLinks[] = "com.copypaste.app/pairing_links";

struct ChannelState {
  GtkApplication* application = nullptr;
  std::array<FlMethodChannel*, 9> channels{};
};

ChannelState* state = nullptr;
std::vector<ChannelState*> engine_states;
std::unique_ptr<LinuxPortal> portal;
std::unique_ptr<LinuxGnomeShortcuts> gnome_shortcuts;
std::unique_ptr<LinuxX11QuickPaste> x11_quick_paste;
LinuxPackageKit packagekit;
GDBusConnection* bridge_connection = nullptr;
GDBusMethodInvocation* bridge_waiter = nullptr;
GDBusMethodInvocation* bridge_begin = nullptr;
GDBusMethodInvocation* bridge_selection = nullptr;
std::string bridge_transaction;
bool bridge_accepted = false;
guint bridge_timeout = 0;
GPid bridge_child_pid = 0;
std::string bridge_sender;
std::string bridge_waiter_owner;

constexpr char kBridgeName[] = "app.copypaste.CopyPaste";
constexpr char kBridgePath[] = "/app/copypaste/WaylandIntegration";
constexpr char kBridgeInterface[] = "app.copypaste.WaylandIntegration";

void bridge_child_exited(GPid pid, gint, gpointer) {
  if (bridge_child_pid == pid) bridge_child_pid = 0;
  g_spawn_close_pid(pid);
}

void stop_bridge_child() {
  const GPid child = bridge_child_pid;
  bridge_child_pid = 0;
  if (child != 0) kill(child, SIGTERM);
}

void cancel_bridge(bool notify_companion = true, bool terminate_child = true) {
  if (bridge_timeout != 0) g_source_remove(bridge_timeout);
  bridge_timeout = 0;
  if (bridge_waiter != nullptr) {
    g_dbus_method_invocation_return_value(bridge_waiter, g_variant_new("(b)", FALSE));
    g_object_unref(bridge_waiter);
    bridge_waiter = nullptr;
  }
  if (bridge_begin != nullptr) {
    g_dbus_method_invocation_return_value(bridge_begin, g_variant_new("(b)", FALSE));
    g_object_unref(bridge_begin);
    bridge_begin = nullptr;
  }
  if (bridge_selection != nullptr) {
    g_dbus_method_invocation_return_value(bridge_selection, g_variant_new("(b)", FALSE));
    g_object_unref(bridge_selection);
    bridge_selection = nullptr;
  }
  if (notify_companion && bridge_connection != nullptr && !bridge_transaction.empty()) {
    g_dbus_connection_emit_signal(
        bridge_connection, bridge_sender.empty() ? nullptr : bridge_sender.c_str(),
        kBridgePath, kBridgeInterface, "TransactionCancelled",
        g_variant_new("(s)", bridge_transaction.c_str()), nullptr);
  }
  bridge_transaction.clear();
  bridge_sender.clear();
  bridge_waiter_owner.clear();
  if (terminate_child) stop_bridge_child();
  bridge_accepted = false;
}

gboolean bridge_expired(gpointer) { cancel_bridge(); return G_SOURCE_REMOVE; }

bool launch_quick_paste(const std::string& transaction) {
  gchar executable[PATH_MAX] = {};
  const ssize_t size = readlink("/proc/self/exe", executable, sizeof(executable) - 1);
  if (size <= 0 || size >= static_cast<ssize_t>(sizeof(executable) - 1)) return false;
  executable[size] = '\0';
  gchar quick[] = "--copypaste-quick-paste";
  gchar transaction_arg[96] = {};
  g_snprintf(transaction_arg, sizeof(transaction_arg), "--copypaste-transaction=%s",
             transaction.c_str());
  const guint64 presentation = ((static_cast<guint64>(g_get_real_time()) &
                                  G_GUINT64_CONSTANT(0x000000007fffffff)) << 32U) |
      static_cast<guint64>(g_random_int());
  gchar presentation_arg[80] = {};
  g_snprintf(presentation_arg, sizeof(presentation_arg),
             "--copypaste-presentation-id=%" G_GUINT64_FORMAT, presentation);
  gchar* argv[] = {executable, quick, transaction_arg, presentation_arg, nullptr};
  if (!g_spawn_async(nullptr, argv, nullptr, static_cast<GSpawnFlags>(
                     G_SPAWN_SEARCH_PATH | G_SPAWN_DO_NOT_REAP_CHILD), nullptr,
                     nullptr, &bridge_child_pid, nullptr)) {
    return false;
  }
  g_child_watch_add(bridge_child_pid, bridge_child_exited, nullptr);
  return true;
}

bool bus_string_call(const gchar* method, const gchar* name, std::string* out) {
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) response = g_dbus_connection_call_sync(
      bridge_connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
      "org.freedesktop.DBus", method, g_variant_new("(s)", name),
      G_VARIANT_TYPE("(s)"), G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
  if (response == nullptr) return false;
  const gchar* value = nullptr;
  g_variant_get(response, "(&s)", &value);
  *out = value == nullptr ? "" : value;
  return true;
}

bool authorised_companion(const gchar* sender) {
  if (sender == nullptr || bridge_connection == nullptr) return false;
  std::string owner;
  const bool gnome = bus_string_call("GetNameOwner", "app.copypaste.GnomeIntegration", &owner) && owner == sender;
  const bool kde = bus_string_call("GetNameOwner", "org.kde.KWin", &owner) && owner == sender;
  if (!gnome && !kde) return false;
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) response = g_dbus_connection_call_sync(
      bridge_connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
      "org.freedesktop.DBus", "GetConnectionUnixUser", g_variant_new("(s)", sender),
      G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
  guint uid = static_cast<guint>(-1);
  if (response == nullptr) return false;
  g_variant_get(response, "(u)", &uid);
  return uid == static_cast<guint>(getuid());
}

bool authorised_child(const gchar* sender) {
  if (sender == nullptr || bridge_connection == nullptr || bridge_child_pid == 0) return false;
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) response = g_dbus_connection_call_sync(
      bridge_connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
      "org.freedesktop.DBus", "GetConnectionUnixProcessID", g_variant_new("(s)", sender),
      G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
  guint pid = 0;
  if (response == nullptr) return false;
  g_variant_get(response, "(u)", &pid);
  return pid == static_cast<guint>(bridge_child_pid);
}

bool is_method(FlMethodCall* call, const char* method) {
  return g_strcmp0(fl_method_call_get_name(call), method) == 0;
}

bool gdk_is_wayland() {
  GdkDisplay* display = gdk_display_get_default();
#if defined(GDK_WINDOWING_WAYLAND)
  return display != nullptr && GDK_IS_WAYLAND_DISPLAY(display);
#else
  return false;
#endif
}

bool gdk_is_x11() {
  GdkDisplay* display = gdk_display_get_default();
#if defined(GDK_WINDOWING_X11)
  return display != nullptr && GDK_IS_X11_DISPLAY(display);
#else
  return false;
#endif
}

void wake_bridge_waiter() {
  if (bridge_waiter == nullptr) return;
  g_dbus_method_invocation_return_value(bridge_waiter, g_variant_new("(b)", TRUE));
  g_object_unref(bridge_waiter);
  bridge_waiter = nullptr;
  bridge_waiter_owner.clear();
}

std::string companion_owner() {
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) connection = g_bus_get_sync(
      G_BUS_TYPE_SESSION, nullptr, &error);
  if (connection == nullptr) return {};
  const char* names[] = {"app.copypaste.GnomeIntegration", "org.kde.KWin"};
  for (const char* name : names) {
    g_clear_error(&error);
    g_autoptr(GVariant) result = g_dbus_connection_call_sync(
        connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
        "org.freedesktop.DBus", "GetNameOwner", g_variant_new("(s)", name),
        G_VARIANT_TYPE("(s)"), G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    if (result != nullptr) {
      const gchar* owner = nullptr;
      g_variant_get(result, "(&s)", &owner);
      if (owner != nullptr && *owner != '\0') return owner;
    }
  }
  return {};
}

bool bridge_companion_active() {
  return bridge_waiter != nullptr && !companion_owner().empty();
}

bool clipboard_version_active() {
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) bus =
      g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
  if (bus == nullptr) return false;
  for (const char* name : {"app.copypaste.GnomeIntegration", "org.kde.KWin"}) {
    g_clear_error(&error);
    g_autoptr(GVariant) owner_reply = g_dbus_connection_call_sync(
        bus, "org.freedesktop.DBus", "/org/freedesktop/DBus",
        "org.freedesktop.DBus", "GetNameOwner", g_variant_new("(s)", name),
        G_VARIANT_TYPE("(s)"), G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    if (owner_reply == nullptr) continue;
    const gchar* owner = nullptr;
    g_variant_get(owner_reply, "(&s)", &owner);
    g_autoptr(GVariant) uid_reply = owner == nullptr
        ? nullptr
        : g_dbus_connection_call_sync(
              bus, "org.freedesktop.DBus", "/org/freedesktop/DBus",
              "org.freedesktop.DBus", "GetConnectionUnixUser",
              g_variant_new("(s)", owner), G_VARIANT_TYPE("(u)"),
              G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    guint uid = 0;
    if (uid_reply == nullptr) continue;
    g_variant_get(uid_reply, "(u)", &uid);
    if (uid != static_cast<guint>(getuid())) continue;
    g_autoptr(GVariant) version = g_dbus_connection_call_sync(
        bus, owner, "/app/copypaste/Clipboard", "app.copypaste.Clipboard",
        "Version", nullptr, G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE,
        500, nullptr, &error);
    guint value = 0;
    if (version == nullptr) continue;
    g_variant_get(version, "(u)", &value);
    if (value != 2) continue;
    g_autoptr(GVariant) current = g_dbus_connection_call_sync(
        bus, "org.freedesktop.DBus", "/org/freedesktop/DBus",
        "org.freedesktop.DBus", "GetNameOwner", g_variant_new("(s)", name),
        G_VARIANT_TYPE("(s)"), G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    const gchar* now = nullptr;
    if (current != nullptr) g_variant_get(current, "(&s)", &now);
    if (g_strcmp0(owner, now) == 0) return true;
  }
  return false;
}

bool clipboard_version_active() {
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
  if (bus == nullptr) return false;
  for (const char* name : {"app.copypaste.GnomeIntegration", "org.kde.KWin"}) {
    g_clear_error(&error);
    g_autoptr(GVariant) owner_reply = g_dbus_connection_call_sync(bus, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner", g_variant_new("(s)", name), G_VARIANT_TYPE("(s)"), G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    if (owner_reply == nullptr) continue;
    const gchar* owner = nullptr; g_variant_get(owner_reply, "(&s)", &owner);
    g_autoptr(GVariant) uid_reply = owner == nullptr ? nullptr : g_dbus_connection_call_sync(bus, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixUser", g_variant_new("(s)", owner), G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    guint uid = 0; if (uid_reply == nullptr) continue; g_variant_get(uid_reply, "(u)", &uid); if (uid != static_cast<guint>(getuid())) continue;
    g_autoptr(GVariant) version = g_dbus_connection_call_sync(bus, owner, "/app/copypaste/Clipboard", "app.copypaste.Clipboard", "Version", nullptr, G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    guint value = 0; if (version == nullptr) continue; g_variant_get(version, "(u)", &value); if (value != 2) continue;
    g_autoptr(GVariant) current = g_dbus_connection_call_sync(bus, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner", g_variant_new("(s)", name), G_VARIANT_TYPE("(s)"), G_DBUS_CALL_FLAGS_NONE, 500, nullptr, &error);
    const gchar* now = nullptr; if (current != nullptr) g_variant_get(current, "(&s)", &now); if (g_strcmp0(owner, now) == 0) return true;
  }
  return false;
}

gint64 integer_argument(FlMethodCall* call, const gchar* name,
                        gint64 fallback = 0) {
  FlValue* args = fl_method_call_get_args(call);
  if (args == nullptr || fl_value_get_type(args) != FL_VALUE_TYPE_MAP) {
    return fallback;
  }
  FlValue* value = fl_value_lookup_string(args, name);
  return value != nullptr && fl_value_get_type(value) == FL_VALUE_TYPE_INT
             ? fl_value_get_int(value)
             : fallback;
}

bool presentation_matches(FlMethodCall* call) {
  const gint64 supplied = integer_argument(call, "presentationId");
  const gchar* presentation = static_cast<const gchar*>(g_object_get_data(
      G_OBJECT(state->application), "copypaste-presentation-id"));
  if (presentation == nullptr || *presentation == '\0') return false;
  gchar* end = nullptr;
  const gint64 current = g_ascii_strtoll(presentation, &end, 10);
  return end != presentation && *end == '\0' && current > 0 && current == supplied;
}

void quit_context_when_idle() {
  if (state == nullptr || state->application == nullptr) return;
  GApplication* application = G_APPLICATION(g_object_ref(state->application));
  g_idle_add_full(
      G_PRIORITY_DEFAULT_IDLE,
      [](gpointer data) {
        auto* app = G_APPLICATION(data);
        g_application_quit(app);
        g_object_unref(app);
        return G_SOURCE_REMOVE;
      },
      application, nullptr);
}

bool spawn_quick_paste(const std::string& x11_window = {}) {
  if (bridge_child_pid != 0) {
    if (kill(bridge_child_pid, 0) == 0) return true;
    bridge_child_pid = 0;
  }
  gchar executable[PATH_MAX] = {};
  const ssize_t size = readlink("/proc/self/exe", executable, sizeof(executable) - 1);
  if (size <= 0 || size >= static_cast<ssize_t>(sizeof(executable) - 1)) return false;
  executable[size] = '\0';
  const guint64 presentation = ((static_cast<guint64>(g_get_real_time()) &
                                  G_GUINT64_CONSTANT(0x000000007fffffff)) << 32U) |
      static_cast<guint64>(g_random_int());
  gchar quick[] = "--copypaste-quick-paste";
  gchar presentation_arg[80] = {};
  g_snprintf(presentation_arg, sizeof(presentation_arg),
             "--copypaste-presentation-id=%" G_GUINT64_FORMAT, presentation);
  gchar x11_arg[80] = {};
  if (!x11_window.empty()) {
    g_snprintf(x11_arg, sizeof(x11_arg), "--copypaste-x11-window=%s",
               x11_window.c_str());
  }
  gchar* arguments[] = {executable, quick, presentation_arg,
                        x11_window.empty() ? nullptr : x11_arg, nullptr};
  if (!g_spawn_async(nullptr, arguments, nullptr, static_cast<GSpawnFlags>(
                     G_SPAWN_SEARCH_PATH | G_SPAWN_DO_NOT_REAP_CHILD),
                     nullptr, nullptr, &bridge_child_pid, nullptr)) {
    return false;
  }
  g_child_watch_add(bridge_child_pid, bridge_child_exited, nullptr);
  return true;
}

bool is_directory_not_link(const gchar* path) {
  struct stat status {};
  return lstat(path, &status) == 0 && S_ISDIR(status.st_mode) &&
      !S_ISLNK(status.st_mode);
}

bool is_regular_not_link(const gchar* path) {
  struct stat status {};
  return lstat(path, &status) == 0 && S_ISREG(status.st_mode) &&
      !S_ISLNK(status.st_mode);
}

bool remove_tree(const gchar* directory) {
  g_autoptr(GDir) entries = g_dir_open(directory, 0, nullptr);
  if (entries == nullptr) return false;
  const gchar* entry = nullptr;
  while ((entry = g_dir_read_name(entries)) != nullptr) {
    g_autofree gchar* path = g_build_filename(directory, entry, nullptr);
    struct stat status {};
    if (lstat(path, &status) != 0) return false;
    if (S_ISDIR(status.st_mode) && !S_ISLNK(status.st_mode)) {
      if (!remove_tree(path)) return false;
    } else if (g_remove(path) != 0) {
      return false;
    }
  }
  return g_rmdir(directory) == 0;
}

bool copy_tree_without_links(const gchar* source, const gchar* destination) {
  if (!is_directory_not_link(source)) return false;
  struct stat destination_status {};
  if (lstat(destination, &destination_status) != 0) {
    if (g_mkdir(destination, 0700) != 0) return false;
  } else if (!S_ISDIR(destination_status.st_mode) || S_ISLNK(destination_status.st_mode)) {
    return false;
  }
  g_autoptr(GDir) entries = g_dir_open(source, 0, nullptr);
  if (entries == nullptr) return false;
  const gchar* entry = nullptr;
  while ((entry = g_dir_read_name(entries)) != nullptr) {
    g_autofree gchar* source_path = g_build_filename(source, entry, nullptr);
    g_autofree gchar* destination_path = g_build_filename(destination, entry, nullptr);
    struct stat status {};
    if (lstat(source_path, &status) != 0 || S_ISLNK(status.st_mode)) return false;
    if (S_ISDIR(status.st_mode)) {
      if (!copy_tree_without_links(source_path, destination_path)) return false;
      continue;
    }
    if (!S_ISREG(status.st_mode)) return false;
    g_autoptr(GFile) source_file = g_file_new_for_path(source_path);
    g_autoptr(GFile) destination_file = g_file_new_for_path(destination_path);
    g_autoptr(GError) error = nullptr;
    if (!g_file_copy(source_file, destination_file, G_FILE_COPY_NONE, nullptr,
                     nullptr, nullptr, &error)) {
      return false;
    }
  }
  return true;
}

bool regular_files_match(const gchar* source, const gchar* target) {
  struct stat source_status {};
  struct stat target_status {};
  if (!is_regular_not_link(source) || !is_regular_not_link(target) ||
      stat(source, &source_status) != 0 || stat(target, &target_status) != 0 ||
      source_status.st_size != target_status.st_size ||
      source_status.st_size > 32 * 1024 * 1024) {
    return false;
  }
  gchar* source_contents = nullptr;
  gchar* target_contents = nullptr;
  gsize source_length = 0;
  gsize target_length = 0;
  const bool loaded =
      g_file_get_contents(source, &source_contents, &source_length, nullptr) &&
      g_file_get_contents(target, &target_contents, &target_length, nullptr);
  std::unique_ptr<gchar, decltype(&g_free)> source_holder(source_contents, g_free);
  std::unique_ptr<gchar, decltype(&g_free)> target_holder(target_contents, g_free);
  return loaded && source_length == target_length &&
      std::memcmp(source_contents, target_contents, source_length) == 0;
}

bool trees_match(const gchar* source, const gchar* target) {
  if (!is_directory_not_link(source) || !is_directory_not_link(target)) return false;
  g_autoptr(GDir) source_entries = g_dir_open(source, 0, nullptr);
  g_autoptr(GDir) target_entries = g_dir_open(target, 0, nullptr);
  if (source_entries == nullptr || target_entries == nullptr) return false;
  const gchar* entry = nullptr;
  while ((entry = g_dir_read_name(source_entries)) != nullptr) {
    g_autofree gchar* source_path = g_build_filename(source, entry, nullptr);
    g_autofree gchar* target_path = g_build_filename(target, entry, nullptr);
    struct stat source_status {};
    struct stat target_status {};
    if (lstat(source_path, &source_status) != 0 ||
        lstat(target_path, &target_status) != 0 ||
        S_ISLNK(source_status.st_mode) || S_ISLNK(target_status.st_mode) ||
        S_ISDIR(source_status.st_mode) != S_ISDIR(target_status.st_mode)) {
      return false;
    }
    if (S_ISDIR(source_status.st_mode)) {
      if (!trees_match(source_path, target_path)) return false;
    } else if (!regular_files_match(source_path, target_path)) {
      return false;
    }
  }
  while ((entry = g_dir_read_name(target_entries)) != nullptr) {
    g_autofree gchar* source_path = g_build_filename(source, entry, nullptr);
    struct stat source_status {};
    if (lstat(source_path, &source_status) != 0) return false;
  }
  return true;
}

bool contains_identity(const gchar* path, const gchar* identity) {
  gchar* contents = nullptr;
  gsize length = 0;
  if (!is_regular_not_link(path) ||
      !g_file_get_contents(path, &contents, &length, nullptr) ||
      length > 64 * 1024) {
    g_free(contents);
    return false;
  }
  std::unique_ptr<gchar, decltype(&g_free)> holder(contents, g_free);
  return g_strstr_len(contents, length, identity) != nullptr;
}

bool target_has_companion_identity(const gchar* target, bool gnome) {
  g_autofree gchar* metadata = gnome
      ? g_build_filename(target, "metadata.json", nullptr)
      : g_build_filename(target, "metadata.desktop", nullptr);
  return contains_identity(metadata, gnome
      ? "copypaste-quick-paste@copypaste.app"
      : "copypaste-quick-paste");
}

bool stage_companion_replace(const gchar* source, const gchar* target,
                             const gchar* parent, bool gnome) {
  struct stat target_status {};
  const bool target_exists = lstat(target, &target_status) == 0;
  if (target_exists && (!S_ISDIR(target_status.st_mode) ||
                        S_ISLNK(target_status.st_mode) ||
                        !target_has_companion_identity(target, gnome))) {
    return false;
  }
  g_autofree gchar* stage = g_build_filename(parent, ".copypaste-stage-XXXXXX", nullptr);
  if (g_mkdtemp(stage) == nullptr || !copy_tree_without_links(source, stage)) {
    remove_tree(stage);
    return false;
  }
  if (!target_exists) {
    const bool installed = g_rename(stage, target) == 0;
    if (!installed) remove_tree(stage);
    return installed;
  }
  g_autofree gchar* backup = g_build_filename(parent, ".copypaste-backup-XXXXXX", nullptr);
  if (g_mkdtemp(backup) == nullptr || g_rmdir(backup) != 0 ||
      g_rename(target, backup) != 0) {
    remove_tree(stage);
    return false;
  }
  if (g_rename(stage, target) != 0) {
    g_rename(backup, target);
    remove_tree(stage);
    return false;
  }
  return remove_tree(backup);
}

bool manifest_is_expected(const gchar* root) {
  g_autofree gchar* manifest = g_build_filename(root, "manifest.json", nullptr);
  gchar* contents = nullptr;
  gsize length = 0;
  if (!is_regular_not_link(manifest) ||
      !g_file_get_contents(manifest, &contents, &length, nullptr) ||
      length == 0 || length > 64 * 1024) {
    g_free(contents);
    return false;
  }
  std::unique_ptr<gchar, decltype(&g_free)> holder(contents, g_free);
  return g_strstr_len(contents, length, "\"schemaVersion\": 1") != nullptr &&
      g_strstr_len(contents, length, "\"gnome-shell-extension\"") != nullptr &&
      g_strstr_len(contents, length, "\"kde-kwin-script\"") != nullptr &&
      g_strstr_len(contents, length, "\"enable\": \"user\"") != nullptr;
}

bool gnome_native_library_matches_architecture(const gchar* source) {
  g_autofree gchar* library = g_build_filename(
      source, "native", "lib", "libcopypaste_clipboard_source.so", nullptr);
  if (!is_regular_not_link(library)) return false;
  gchar* contents = nullptr;
  gsize length = 0;
  if (!g_file_get_contents(library, &contents, &length, nullptr) || length < 20) {
    g_free(contents);
    return false;
  }
  std::unique_ptr<gchar, decltype(&g_free)> holder(contents, g_free);
  const auto* bytes = reinterpret_cast<const guint8*>(contents);
  if (bytes[0] != 0x7f || bytes[1] != 'E' || bytes[2] != 'L' || bytes[3] != 'F' ||
      bytes[5] != 1) {
    return false;
  }
  const guint16 machine = static_cast<guint16>(bytes[18]) |
      (static_cast<guint16>(bytes[19]) << 8U);
#if defined(__aarch64__)
  return machine == 183;
#else
  return machine == 62;
#endif
}

bool install_appimage_companion(const gchar* appdir, const gchar* desktop) {
  g_autofree gchar* root = g_build_filename(
      appdir, "usr", "share", "copypaste", "desktop-integrations", nullptr);
  if (!manifest_is_expected(root)) return false;
  const bool gnome = g_strrstr(desktop, "gnome") != nullptr;
  const bool kde = g_strrstr(desktop, "kde") != nullptr ||
      g_strrstr(desktop, "plasma") != nullptr;
  if (!gnome && !kde) return false;
  const gchar* source_name = gnome ? "gnome-shell-extension" : "kde-kwin-script";
  const gchar* target_root = gnome ? "gnome-shell/extensions" : "kwin/scripts";
  const gchar* target_name = gnome ? "copypaste-quick-paste@copypaste.app"
                                   : "copypaste-quick-paste";
  g_autofree gchar* source = g_build_filename(root, source_name, nullptr);
  if (!is_directory_not_link(source) || (gnome &&
      !gnome_native_library_matches_architecture(source))) {
    return false;
  }
  g_autofree gchar* parent = g_build_filename(g_get_user_data_dir(), target_root, nullptr);
  if (g_mkdir_with_parents(parent, 0700) != 0) return false;
  g_autofree gchar* target = g_build_filename(parent, target_name, nullptr);
  struct stat target_status {};
  if (lstat(target, &target_status) == 0) {
    if (!S_ISDIR(target_status.st_mode) || S_ISLNK(target_status.st_mode)) {
      return false;
    }
    if (trees_match(source, target)) return true;
  }
  return stage_companion_replace(source, target, parent, gnome);
}

bool spawn_companion_setup() {
  if (!gdk_is_wayland()) return false;
  const gchar* desktop = g_getenv("XDG_CURRENT_DESKTOP");
  g_autofree gchar* lowered = desktop == nullptr ? nullptr : g_ascii_strdown(desktop, -1);
  if (lowered == nullptr) return false;
  const gchar* appdir = g_getenv("APPDIR");
  if (appdir != nullptr && *appdir != '\0' &&
      !install_appimage_companion(appdir, lowered)) {
    return false;
  }
  if (lowered != nullptr && g_strrstr(lowered, "gnome") != nullptr) {
    gchar* args[] = {const_cast<gchar*>("gnome-extensions-app"), nullptr};
    return g_spawn_async(nullptr, args, nullptr, G_SPAWN_SEARCH_PATH, nullptr,
                         nullptr, nullptr, nullptr);
  }
  if (lowered != nullptr && (g_strrstr(lowered, "kde") != nullptr ||
                             g_strrstr(lowered, "plasma") != nullptr)) {
    gchar settings[] = "systemsettings";
    gchar page[] = "kcm_kwin_scripts";
    gchar* args[] = {settings, page, nullptr};
    return g_spawn_async(nullptr, args, nullptr, G_SPAWN_SEARCH_PATH, nullptr,
                         nullptr, nullptr, nullptr);
  }
  return false;
}

void present_root_window() {
  if (state == nullptr || state->application == nullptr) return;
  GList* windows = gtk_application_get_windows(state->application);
  if (windows != nullptr) gtk_window_present(GTK_WINDOW(windows->data));
}

bool open_root_surface(bool settings) {
  present_root_window();
  if (!settings || engine_states.empty() || engine_states.front()->channels[2] == nullptr) {
    return !settings;
  }
  fl_method_channel_invoke_method(engine_states.front()->channels[2], "openSettings",
                                  nullptr, nullptr, nullptr, nullptr);
  return true;
}

void success(FlMethodCall* call, FlValue* result = nullptr) {
  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond_success(call, result, &error)) {
    g_warning("Failed to answer native platform call: %s", error->message);
  }
  if (result != nullptr) fl_value_unref(result);
}

void failure(FlMethodCall* call, const char* code, const char* message) {
  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond_error(call, code, message, nullptr, &error)) {
    g_warning("Failed to answer native platform call: %s", error->message);
  }
}

void unsupported(FlMethodCall* call) {
  fl_method_call_respond_not_implemented(call, nullptr);
}

bool usable_appimage_target() {
  const gchar* appimage = g_getenv("APPIMAGE");
  const gchar* appdir = g_getenv("APPDIR");
  struct stat target_status {};
  struct stat appdir_status {};
  if (appimage == nullptr || appdir == nullptr ||
      lstat(appimage, &target_status) != 0 || !S_ISREG(target_status.st_mode) ||
      target_status.st_uid != getuid() || g_access(appimage, W_OK) != 0 ||
      lstat(appdir, &appdir_status) != 0 || !S_ISDIR(appdir_status.st_mode) ||
      S_ISLNK(appdir_status.st_mode)) {
    return false;
  }
  gchar executable[PATH_MAX] = {};
  const ssize_t executable_length =
      readlink("/proc/self/exe", executable, sizeof(executable) - 1);
  if (executable_length <= 0 ||
      executable_length >= static_cast<ssize_t>(sizeof(executable) - 1)) {
    return false;
  }
  executable[executable_length] = '\0';
  g_autofree gchar* expected = g_build_filename(
      appdir, "usr", "lib", "copypaste", "copypaste", nullptr);
  struct stat expected_status {};
  if (lstat(expected, &expected_status) != 0 || !S_ISREG(expected_status.st_mode) ||
      S_ISLNK(expected_status.st_mode)) {
    return false;
  }
  g_autofree gchar* canonical_target = g_canonicalize_filename(expected, nullptr);
  g_autofree gchar* canonical_executable =
      g_canonicalize_filename(executable, nullptr);
  return g_strcmp0(canonical_target, canonical_executable) == 0;
}

FlValue* update_availability() {
  FlValue* response = fl_value_new_map();
  const bool writable_appimage = usable_appimage_target();
  fl_value_set_string_take(response, "available",
                           fl_value_new_bool(writable_appimage));
  if (writable_appimage) {
    fl_value_set_string_take(response, "installationType",
                             fl_value_new_string("appimage"));
#if defined(__aarch64__)
    fl_value_set_string_take(response, "architecture",
                             fl_value_new_string("aarch64"));
#else
    fl_value_set_string_take(response, "architecture",
                             fl_value_new_string("x86_64"));
#endif
  } else {
    fl_value_set_string_take(
        response, "reason",
        fl_value_new_string(
            "Install CopyPaste as a writable AppImage to update it here."));
  }
  return response;
}

const gchar* string_argument(FlMethodCall* call, const gchar* name) {
  FlValue* args = fl_method_call_get_args(call);
  if (args == nullptr || fl_value_get_type(args) != FL_VALUE_TYPE_MAP) {
    return nullptr;
  }
  FlValue* value = fl_value_lookup_string(args, name);
  return value != nullptr && fl_value_get_type(value) == FL_VALUE_TYPE_STRING
             ? fl_value_get_string(value)
             : nullptr;
}

bool valid_sha256(const gchar* value) {
  if (value == nullptr || std::strlen(value) != 64) {
    return false;
  }
  for (const gchar* current = value; *current != '\0'; current++) {
    if (!g_ascii_isxdigit(*current)) {
      return false;
    }
  }
  return true;
}

bool matches_sha256(const gchar* path, const gchar* expected) {
  if (!valid_sha256(expected)) {
    return false;
  }
  g_autoptr(GChecksum) checksum = g_checksum_new(G_CHECKSUM_SHA256);
  g_autoptr(GFile) file = g_file_new_for_path(path);
  g_autoptr(GFileInputStream) input = g_file_read(file, nullptr, nullptr);
  if (input == nullptr) {
    return false;
  }
  std::array<guint8, 64 * 1024> buffer{};
  while (true) {
    g_autoptr(GError) error = nullptr;
    const gssize count = g_input_stream_read(G_INPUT_STREAM(input), buffer.data(),
                                             buffer.size(), nullptr, &error);
    if (count < 0) {
      return false;
    }
    if (count == 0) {
      break;
    }
    g_checksum_update(checksum, buffer.data(), static_cast<gsize>(count));
  }
  return g_ascii_strcasecmp(g_checksum_get_string(checksum), expected) == 0;
}

bool install_appimage(const gchar* source, const gchar* expected_sha256) {
  const gchar* target = g_getenv("APPIMAGE");
  struct stat source_status {};
  if (!usable_appimage_target() || target == nullptr ||
      lstat(source, &source_status) != 0 || !S_ISREG(source_status.st_mode) ||
      !matches_sha256(source, expected_sha256)) {
    return false;
  }
  g_autofree gchar* directory = g_path_get_dirname(target);
  g_autofree gchar* staging =
      g_build_filename(directory, ".copypaste-update-XXXXXX", nullptr);
  const int staging_fd = g_mkstemp(staging);
  if (staging_fd < 0) return false;
  g_autoptr(GFile) source_file = g_file_new_for_path(source);
  g_autoptr(GFile) staging_file = g_file_new_for_path(staging);
  g_autoptr(GError) error = nullptr;
  close(staging_fd);
  // g_mkstemp creates a non-predictable exclusive name. Remove that empty
  // inode before the GLib copy; no-overwrite copy and the staged digest both
  // reject a competing file or changed source before rename.
  if (g_remove(staging) != 0 ||
      !g_file_copy(source_file, staging_file, G_FILE_COPY_NONE, nullptr,
                   nullptr, nullptr, &error) ||
      g_chmod(staging, 0755) != 0) {
    g_remove(staging);
    return false;
  }
  const int staged_fd = open(staging, O_RDONLY | O_CLOEXEC);
  bool staged_synced = staged_fd >= 0;
  if (staged_fd >= 0) {
    staged_synced = fsync(staged_fd) == 0;
    staged_synced = close(staged_fd) == 0 && staged_synced;
  }
  if (!staged_synced || !matches_sha256(staging, expected_sha256)) {
    g_remove(staging);
    return false;
  }
  // Both names are in the AppImage directory: rename is atomic and never
  // evaluates input as a shell command. The running process retains its inode
  // until restart.
  if (g_rename(staging, target) != 0) {
    g_remove(staging);
    return false;
  }
  return true;
}

FlValue* integration_status() {
  const bool x11 = gdk_is_x11() && x11_quick_paste != nullptr &&
      x11_quick_paste->available();
  const bool wayland = gdk_is_wayland();
  // The Clipboard v2 payload contract admits only the private daemon owner.
  // The GUI must not probe Version or Snapshot as a readiness shortcut: that
  // would either fail closed or require widening a payload-bearing interface.
  // Wayland remains unavailable until the daemon supplies authenticated public
  // readiness metadata through its typed runtime status.
  const bool clipboard = x11 || (wayland && clipboard_version_active());
  const bool companion = wayland && clipboard;
  const bool portal_shortcuts = portal != nullptr && portal->is_available();
  const bool gnome_shortcuts_available = wayland && !portal_shortcuts &&
      gnome_shortcuts != nullptr && gnome_shortcuts->is_available();
  const bool remote_active = portal != nullptr && portal->remote_desktop_active();
  const bool remote_available = portal != nullptr && portal->remote_desktop_available();
  const bool shortcut_active = (portal_shortcuts && portal->shortcut_registered()) ||
      (gnome_shortcuts_available && gnome_shortcuts->shortcut_registered());
  const char* session = wayland ? "wayland" : x11 ? "x11" : "unsupported";
  FlValue* response = fl_value_new_map();
  fl_value_set_string_take(response, "session", fl_value_new_string(session));
  fl_value_set_string_take(response, "globalShortcuts",
                           fl_value_new_bool(x11 || portal_shortcuts ||
                                             gnome_shortcuts_available));
  fl_value_set_string_take(response, "remoteDesktop",
                           fl_value_new_string(wayland
                               ? (remote_active ? "active" : remote_available
                                   ? "consentRequired" : "unsupported")
                               : "unsupported"));
  fl_value_set_string_take(response, "companion", fl_value_new_string(
      companion ? "active" : "unavailable"));
  fl_value_set_string_take(response, "clipboard", fl_value_new_bool(
      clipboard));
  fl_value_set_string_take(response, "quickPaste", fl_value_new_bool(
      x11 ? x11_quick_paste->input_available()
          : wayland && companion && shortcut_active && remote_active));
  // Wayland and X11 do not provide a GTK client API for compositor-wide
  // screenshot exclusion. The UI must keep this unsupported rather than fake
  // a protected pairing context.
  fl_value_set_string_take(response, "screenshotProtection",
                           fl_value_new_bool(false));
  return response;
}

void security_call(FlMethodChannel*, FlMethodCall* call, gpointer) {
  if (is_method(call, "getBlockScreenshots") ||
      is_method(call, "setBlockScreenshots")) {
    success(call, fl_value_new_bool(false));
    return;
  }
  unsupported(call);
}

void pairing_call(FlMethodChannel*, FlMethodCall* call, gpointer) {
  if (is_method(call, "isSupported")) {
    success(call, fl_value_new_bool(false));
    return;
  }
  if (is_method(call, "setCaptureProtection")) {
    FlValue* args = fl_method_call_get_args(call);
    FlValue* enabled = args != nullptr && fl_value_get_type(args) == FL_VALUE_TYPE_MAP
                           ? fl_value_lookup_string(args, "enabled")
                           : nullptr;
    if (enabled == nullptr || fl_value_get_type(enabled) != FL_VALUE_TYPE_BOOL) {
      failure(call, "invalid_arguments", "Capture protection details are invalid.");
      return;
    }
    // Linux reports screenshot protection unavailable, but the existing
    // pairing inspector remains usable. This acknowledges that its requested
    // policy has been reconciled with the explicit platform exception.
    success(call, fl_value_new_bool(state != nullptr && state->application != nullptr &&
        gtk_application_get_windows(state->application) != nullptr));
    return;
  }
  unsupported(call);
}

void quick_paste_call(FlMethodChannel*, FlMethodCall* call, gpointer) {
  const bool x11 = gdk_is_x11() && x11_quick_paste != nullptr &&
      x11_quick_paste->input_available();
  const bool portal_shortcuts = portal != nullptr && portal->is_available();
  const bool gnome_shortcuts_available = gdk_is_wayland() && !portal_shortcuts &&
      gnome_shortcuts != nullptr && gnome_shortcuts->is_available();
  const bool wayland_ready = gdk_is_wayland() && bridge_companion_active() &&
      ((portal_shortcuts && portal->shortcut_registered()) ||
       (gnome_shortcuts_available && gnome_shortcuts->shortcut_registered()));
  if (is_method(call, "isSupported") || is_method(call, "prepare")) {
    success(call, fl_value_new_bool(x11 || wayland_ready));
  } else if (is_method(call, "accessibilityGranted") ||
             is_method(call, "requestAccessibility")) {
    success(call, fl_value_new_bool(x11 || (portal != nullptr &&
                                            portal->remote_desktop_active())));
  } else if (is_method(call, "open")) {
    success(call, fl_value_new_bool(x11 && spawn_quick_paste(
        x11_quick_paste->focused_window())));
  } else if (is_method(call, "dispose")) {
    success(call, fl_value_new_bool(true));
  } else {
    unsupported(call);
  }
}

void quick_paste_context_call(FlMethodChannel* channel, FlMethodCall* call, gpointer) {
  if (is_method(call, "paste")) {
    if (!presentation_matches(call)) {
      success(call, fl_value_new_bool(false));
      return;
    }
    const gchar* x11_window = static_cast<const gchar*>(g_object_get_data(
        G_OBJECT(state->application), "copypaste-x11-window"));
    if (x11_window != nullptr && x11_quick_paste != nullptr) {
      const bool pasted = x11_quick_paste->restore_and_paste(x11_window);
      success(call, fl_value_new_bool(pasted));
      if (pasted) quit_context_when_idle();
      return;
    }
    const gchar* transaction = static_cast<const gchar*>(g_object_get_data(
        G_OBJECT(state->application), "copypaste-transaction"));
    if (transaction == nullptr || *transaction == '\0') {
      success(call, fl_value_new_bool(false));
      return;
    }
    g_autoptr(GError) error = nullptr;
    g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
    g_autoptr(GVariant) result = bus == nullptr ? nullptr : g_dbus_connection_call_sync(
        bus, kBridgeName, kBridgePath, kBridgeInterface, "SelectionCommitted",
        g_variant_new("(s)", transaction), G_VARIANT_TYPE("(b)"),
        G_DBUS_CALL_FLAGS_NONE, 2000, nullptr, &error);
    gboolean acknowledged = FALSE;
    if (result != nullptr) g_variant_get(result, "(b)", &acknowledged);
    success(call, fl_value_new_bool(acknowledged));
    if (acknowledged) quit_context_when_idle();
  } else if (is_method(call, "setInspectorVisible")) {
    FlValue* args = fl_method_call_get_args(call);
    FlValue* visible = args != nullptr && fl_value_get_type(args) == FL_VALUE_TYPE_MAP
        ? fl_value_lookup_string(args, "visible") : nullptr;
    if (!presentation_matches(call) || visible == nullptr ||
        fl_value_get_type(visible) != FL_VALUE_TYPE_BOOL) {
      success(call, fl_value_new_bool(false));
      return;
    }
    GtkWindow* context = nullptr;
    for (GList* item = gtk_application_get_windows(state->application);
         item != nullptr; item = item->next) {
      GtkWindow* window = GTK_WINDOW(item->data);
      if (g_strcmp0(gtk_window_get_role(window), "copypaste-quick-paste") == 0) {
        context = window;
        break;
      }
    }
    const bool resized = resize_linux_quick_paste_window(
        context, fl_value_get_bool(visible));
    success(call, fl_value_new_bool(resized && presentation_matches(call)));
  } else if (is_method(call, "accessibilityGranted") ||
             is_method(call, "requestAccessibility")) {
    const bool x11 = x11_quick_paste != nullptr && x11_quick_paste->input_available();
    const gchar* transaction = static_cast<const gchar*>(g_object_get_data(
        G_OBJECT(state->application), "copypaste-transaction"));
    g_autoptr(GError) error = nullptr;
    g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
    g_autoptr(GVariant) result = bus == nullptr || transaction == nullptr ? nullptr :
        g_dbus_connection_call_sync(bus, kBridgeName, kBridgePath, kBridgeInterface,
                                    "QuickPasteInputReady", g_variant_new("(s)", transaction),
                                    G_VARIANT_TYPE("(b)"), G_DBUS_CALL_FLAGS_NONE,
                                    2000, nullptr, &error);
    gboolean ready = FALSE;
    if (result != nullptr) g_variant_get(result, "(b)", &ready);
    success(call, fl_value_new_bool(x11 || ready));
  } else if (is_method(call, "ready")) {
    const gchar* presentation = static_cast<const gchar*>(g_object_get_data(
        G_OBJECT(state->application), "copypaste-presentation-id"));
    gchar* end = nullptr;
    const gint64 presentation_id = presentation == nullptr ? 0 :
        g_ascii_strtoll(presentation, &end, 10);
    if (presentation_id <= 0 || end == presentation || *end != '\0') {
      success(call, fl_value_new_bool(false));
      return;
    }
    success(call, fl_value_new_bool(true));
    FlValue* opened = fl_value_new_map();
    fl_value_set_string_take(opened, "presentationId", fl_value_new_int(presentation_id));
    fl_value_set_string_take(opened, "inspectorVisible", fl_value_new_bool(false));
    fl_method_channel_invoke_method(channel, "opened", opened, nullptr, nullptr, nullptr);
    fl_value_unref(opened);
  } else if (is_method(call, "close") || is_method(call, "quit")) {
    if (is_method(call, "quit") || presentation_matches(call)) {
      const gchar* transaction = static_cast<const gchar*>(g_object_get_data(
          G_OBJECT(state->application), "copypaste-transaction"));
      const gchar* x11_window = static_cast<const gchar*>(g_object_get_data(
          G_OBJECT(state->application), "copypaste-x11-window"));
      if (x11_window != nullptr && x11_quick_paste != nullptr) {
        x11_quick_paste->restore_focus(x11_window);
      } else if (transaction != nullptr && *transaction != '\0') {
        g_autoptr(GError) error = nullptr;
        g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
        if (bus != nullptr) {
          g_autoptr(GVariant) ignored = g_dbus_connection_call_sync(
              bus, kBridgeName, kBridgePath, kBridgeInterface, "CancelQuickPaste",
              g_variant_new("(s)", transaction), G_VARIANT_TYPE("(b)"),
              G_DBUS_CALL_FLAGS_NONE, 2000, nullptr, &error);
        }
      }
      success(call);
      quit_context_when_idle();
    } else {
      success(call);
    }
  } else if (is_method(call, "openMain") || is_method(call, "openSettings")) {
    const gchar* transaction = static_cast<const gchar*>(g_object_get_data(
        G_OBJECT(state->application), "copypaste-transaction"));
    g_autoptr(GError) error = nullptr;
    g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
    const char* method = is_method(call, "openSettings") ? "OpenSettings" : "OpenMain";
    g_autoptr(GVariant) result = bus == nullptr ? nullptr : g_dbus_connection_call_sync(
        bus, kBridgeName, kBridgePath, kBridgeInterface, method,
        g_variant_new("(s)", transaction == nullptr ? "" : transaction),
        G_VARIANT_TYPE("(b)"), G_DBUS_CALL_FLAGS_NONE, 2000, nullptr, &error);
    gboolean opened = FALSE;
    if (result != nullptr) g_variant_get(result, "(b)", &opened);
    success(call);
    if (opened) quit_context_when_idle();
  } else {
    unsupported(call);
  }
}

void shortcuts_call(FlMethodChannel* channel, FlMethodCall* call, gpointer) {
  const bool x11 = gdk_is_x11() && x11_quick_paste != nullptr &&
      x11_quick_paste->available();
  const bool portal_shortcuts = portal != nullptr && portal->is_available();
  const bool gnome_shortcuts_available = gdk_is_wayland() && !portal_shortcuts &&
      gnome_shortcuts != nullptr && gnome_shortcuts->is_available();
  if (is_method(call, "isSupported")) {
    success(call, fl_value_new_bool(x11 || portal_shortcuts || gnome_shortcuts_available));
  } else if (is_method(call, "register")) {
    ShortcutRequest request;
    const gchar* id = string_argument(call, "id");
    const gchar* description = string_argument(call, "description");
    const gchar* preferred = string_argument(call, "preferredTrigger");
    const gint64 usage = integer_argument(call, "usage", -1);
    FlValue* args = fl_method_call_get_args(call);
    FlValue* modifiers = args == nullptr ? nullptr : fl_value_lookup_string(args, "modifiers");
    if (id == nullptr || description == nullptr || usage < 0 || modifiers == nullptr ||
        fl_value_get_type(modifiers) != FL_VALUE_TYPE_LIST) {
      failure(call, "invalid_arguments", "Quick Paste shortcut details are invalid.");
      return;
    }
    request.id = id;
    request.description = description;
    request.usage = std::to_string(usage);
    if (preferred != nullptr) request.preferred_trigger = preferred;
    for (size_t index = 0; index < fl_value_get_length(modifiers); ++index) {
      FlValue* value = fl_value_get_list_value(modifiers, index);
      if (value == nullptr || fl_value_get_type(value) != FL_VALUE_TYPE_STRING) {
        failure(call, "invalid_arguments", "Quick Paste shortcut details are invalid.");
        return;
      }
      request.modifiers.emplace_back(fl_value_get_string(value));
    }
    FlMethodCall* retained = FL_METHOD_CALL(g_object_ref(call));
    const auto complete = [retained](ShortcutResult result) {
      FlValue* response = fl_value_new_map();
      fl_value_set_string_take(response, "registered", fl_value_new_bool(result.registered));
      if (result.registered) {
        fl_value_set_string_take(response, "triggerDescription",
                                 fl_value_new_string(result.trigger_description.c_str()));
      } else {
        fl_value_set_string_take(response, "reason", fl_value_new_string(result.reason.c_str()));
      }
      success(retained, response);
      g_object_unref(retained);
    };
    if (x11) {
      x11_quick_paste->register_shortcut(request, complete, [channel, id = request.id]() {
        FlValue* arguments = fl_value_new_map();
        fl_value_set_string_take(arguments, "id", fl_value_new_string(id.c_str()));
        fl_method_channel_invoke_method(channel, "activated", arguments, nullptr,
                                        nullptr, nullptr);
        fl_value_unref(arguments);
      });
    } else if (portal_shortcuts && gnome_shortcuts != nullptr &&
               gnome_shortcuts->shortcut_registered()) {
      // The portal became available after the fallback was registered. Release
      // the fallback first so the same shortcut is never owned by both paths.
      gnome_shortcuts->unregister_shortcut(
          request.id, [request, complete](bool removed) mutable {
            if (!removed || portal == nullptr) {
              complete({false, {}, "GNOME did not release the existing shortcut."});
              return;
            }
            portal->register_shortcut(request, std::move(complete));
          });
    } else if (portal_shortcuts) {
      // The Wayland portal callback wakes the authenticated companion's
      // AwaitQuickPaste transaction. It must not open a second Flutter child.
      portal->register_shortcut(request, complete);
    } else if (gnome_shortcuts_available) {
      gnome_shortcuts->register_shortcut(request, complete);
    } else {
      complete({false, {}, "Global shortcuts are unavailable."});
    }
  } else if (is_method(call, "unregister")) {
    const gchar* id = string_argument(call, "id");
    if (id == nullptr) {
      failure(call, "invalid_arguments", "Quick Paste shortcut id is required.");
      return;
    }
    FlMethodCall* retained = FL_METHOD_CALL(g_object_ref(call));
    const auto complete = [retained](bool removed) {
      success(retained, fl_value_new_bool(removed));
      g_object_unref(retained);
    };
    if (x11) {
      x11_quick_paste->unregister_shortcut(id, complete);
    } else if (gnome_shortcuts != nullptr && gnome_shortcuts->shortcut_registered()) {
      gnome_shortcuts->unregister_shortcut(id, complete);
    } else if (portal_shortcuts) {
      portal->unregister_shortcut(id, complete);
    } else if (gnome_shortcuts_available) {
      gnome_shortcuts->unregister_shortcut(id, complete);
    } else {
      complete(false);
    }
  } else {
    unsupported(call);
  }
}

void integration_call(FlMethodChannel*, FlMethodCall* call, gpointer) {
  if (is_method(call, "status")) {
    FlValue* status = integration_status();
    g_autoptr(GError) error = nullptr;
    const auto startup = LinuxXdgStartup::CreateForCurrentExecutable(&error);
    const LinuxXdgStartupStatus startup_status =
        startup ? startup->Status() : LinuxXdgStartupStatus{};
    fl_value_set_string_take(status, "startAtLogin",
                             fl_value_new_bool(startup_status.start_at_login));
    fl_value_set_string_take(status, "uriRegistered",
                             fl_value_new_bool(startup_status.uri_registered));
    success(call, status);
  } else if (is_method(call, "setStartAtLogin")) {
    FlValue* args = fl_method_call_get_args(call);
    FlValue* enabled = args != nullptr && fl_value_get_type(args) == FL_VALUE_TYPE_MAP
        ? fl_value_lookup_string(args, "enabled") : nullptr;
    if (enabled == nullptr || fl_value_get_type(enabled) != FL_VALUE_TYPE_BOOL) {
      failure(call, "invalid_arguments", "Start at login requires an enabled value.");
      return;
    }
    g_autoptr(GError) error = nullptr;
    const auto startup = LinuxXdgStartup::CreateForCurrentExecutable(&error);
    if (!startup || !startup->SetStartAtLogin(fl_value_get_bool(enabled), &error)) {
      failure(call, "xdg_failed", error == nullptr ? "XDG startup is unavailable."
                                                     : error->message);
      return;
    }
    success(call, fl_value_new_bool(true));
  } else if (is_method(call, "registerCopypasteUri")) {
    g_autoptr(GError) error = nullptr;
    const auto startup = LinuxXdgStartup::CreateForCurrentExecutable(&error);
    if (!startup || !startup->RegisterCopypasteUri(&error)) {
      failure(call, "xdg_failed", error == nullptr ? "XDG URI registration is unavailable."
                                                     : error->message);
      return;
    }
    success(call, fl_value_new_bool(true));
  } else if (is_method(call, "openCompanionSetup")) {
    success(call, fl_value_new_bool(spawn_companion_setup()));
  } else if (is_method(call, "requestRemoteDesktop")) {
    if (portal == nullptr) {
      success(call, fl_value_new_bool(false));
      return;
    }
    // Portal completion is asynchronous; retain the Flutter method call until
    // user consent resolves instead of reporting an optimistic grant.
    FlMethodCall* retained = FL_METHOD_CALL(g_object_ref(call));
    portal->request_remote_desktop([retained](bool granted) {
      success(retained, fl_value_new_bool(granted));
      g_object_unref(retained);
    });
  } else {
    unsupported(call);
  }
}

void update_call(FlMethodChannel*, FlMethodCall* call, gpointer) {
  if (is_method(call, "currentVersion")) {
    success(call, fl_value_new_string(COPYPASTE_VERSION));
  } else if (is_method(call, "systemVersion")) {
    const auto version = linux_glibc_version();
    if (!version) {
      failure(call, "system_version_unavailable",
              "The GNU libc version is unavailable.");
      return;
    }
    const std::string value = version->ToString();
    success(call, fl_value_new_string(value.c_str()));
  } else if (is_method(call, "availability")) {
    if (g_getenv("APPIMAGE") != nullptr) {
      success(call, update_availability());
    } else {
      const LinuxPackageInstallation installation = packagekit.Detect();
      FlValue* response = fl_value_new_map();
      fl_value_set_string_take(response, "available",
                               fl_value_new_bool(installation.available()));
      if (installation.available()) {
        fl_value_set_string_take(response, "installationType", fl_value_new_string(
            installation.kind == LinuxPackageKind::kDeb ? "deb" : "rpm"));
        fl_value_set_string_take(response, "architecture",
                                 fl_value_new_string(installation.architecture.c_str()));
      } else {
        fl_value_set_string_take(response, "reason",
                                 fl_value_new_string(installation.reason.c_str()));
      }
      success(call, response);
    }
  } else if (is_method(call, "install")) {
    const gchar* source = string_argument(call, "path");
    const gchar* sha256 = string_argument(call, "sha256");
    if (source == nullptr || sha256 == nullptr) {
      failure(call, "invalid_arguments", "Update package details are invalid.");
    } else if (g_getenv("APPIMAGE") == nullptr) {
      FlMethodCall* retained = FL_METHOD_CALL(g_object_ref(call));
      packagekit.Install(source, sha256, [retained](LinuxPackageInstallResult result,
                                                    std::string message) {
        if (result == LinuxPackageInstallResult::kRestartRequired) {
          success(retained, fl_value_new_string("restart_required"));
        } else if (result == LinuxPackageInstallResult::kPermissionRequired) {
          success(retained, fl_value_new_string("permission_required"));
        } else {
          failure(retained, "packagekit_failed", message.c_str());
        }
        g_object_unref(retained);
      });
    } else if (!install_appimage(source, sha256)) {
      failure(call, "verification_failed",
              "CopyPaste could not verify and stage this AppImage update.");
    } else {
      success(call, fl_value_new_string("restart_required"));
    }
  } else if (is_method(call, "restoreInstallation")) {
    success(call);
  } else if (is_method(call, "openReleasePage")) {
    const gchar* url = string_argument(call, "url");
    if (url == nullptr) {
      failure(call, "invalid_arguments", "Release page URL is required.");
      return;
    }
    g_autoptr(GError) error = nullptr;
    if (!is_linux_release_page_url(url)) {
      failure(call, "open_failed", "Release page URL is not trusted.");
      return;
    }
    if (!g_app_info_launch_default_for_uri(url, nullptr, &error)) {
      failure(call, "open_failed", error == nullptr ? "Could not open release page." : error->message);
      return;
    }
    success(call);
  } else {
    unsupported(call);
  }
}

void lifecycle_call(FlMethodChannel*, FlMethodCall* call, gpointer) {
  if (!is_method(call, "restart")) {
    unsupported(call);
    return;
  }
  if (state == nullptr || state->application == nullptr ||
      g_object_get_data(G_OBJECT(state->application), "copypaste-quick-paste") != nullptr) {
    failure(call, "restart_failed", "Quick Paste cannot restart the primary application.");
    return;
  }
  g_autoptr(GError) error = nullptr;
  if (!linux_restart_helper_schedule_current(&error)) {
    failure(call, "restart_failed", error->message);
    return;
  }
  success(call);
  g_application_quit(G_APPLICATION(state->application));
}

void pairing_links_call(FlMethodChannel*, FlMethodCall* call, gpointer) {
  if (!is_method(call, "takePendingUri")) {
    unsupported(call);
    return;
  }
  const gchar* pending = static_cast<const gchar*>(g_object_get_data(
      G_OBJECT(state->application), "copypaste-pairing-uri"));
  if (pending == nullptr) {
    success(call);
    return;
  }
  success(call, fl_value_new_string(pending));
  g_object_set_data(G_OBJECT(state->application), "copypaste-pairing-uri",
                    nullptr);
}

void bridge_call(GDBusConnection*, const gchar* sender, const gchar*, const gchar*,
                 const gchar* method, GVariant* parameters,
                 GDBusMethodInvocation* invocation, gpointer) {
  if (g_strcmp0(method, "OpenMain") == 0 ||
      g_strcmp0(method, "OpenSettings") == 0) {
    if (!authorised_child(sender)) {
      g_dbus_method_invocation_return_dbus_error(
          invocation, "app.copypaste.WaylandIntegration.Error.Unauthorized",
          "Unauthorized Quick Paste context.");
      return;
    }
    const gchar* transaction = nullptr;
    g_variant_get(parameters, "(&s)", &transaction);
    if (transaction == nullptr ||
        (bridge_transaction.empty() ? *transaction != '\0'
                                    : bridge_transaction != transaction)) {
      g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", FALSE));
      return;
    }
    const bool settings = g_strcmp0(method, "OpenSettings") == 0;
    const bool opened = open_root_surface(settings);
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", opened));
    if (opened) cancel_bridge(true, false);
    return;
  }
  if (g_strcmp0(method, "QuickPasteInputReady") == 0) {
    const gchar* transaction = nullptr;
    g_variant_get(parameters, "(&s)", &transaction);
    const bool ready = transaction != nullptr && bridge_transaction == transaction &&
        authorised_child(sender) && portal != nullptr && portal->remote_desktop_active();
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", ready));
    return;
  }
  if (g_strcmp0(method, "AwaitQuickPaste") == 0) {
    if (!authorised_companion(sender)) {
      g_dbus_method_invocation_return_dbus_error(invocation,
        "app.copypaste.WaylandIntegration.Error.Unauthorized", "Unauthorized companion.");
      return;
    }
    if (bridge_waiter != nullptr) {
      g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", FALSE));
      return;
    }
    bridge_waiter = G_DBUS_METHOD_INVOCATION(g_object_ref(invocation));
    bridge_waiter_owner = sender;
    return;
  }
  const gchar* transaction = nullptr;
  g_variant_get(parameters, "(&s)", &transaction);
  if (transaction == nullptr || *transaction == '\0') {
    g_dbus_method_invocation_return_dbus_error(invocation,
      "app.copypaste.WaylandIntegration.Error.InvalidTransaction", "Invalid transaction.");
    return;
  }
  if (g_strcmp0(method, "BeginQuickPaste") == 0) {
    if (!authorised_companion(sender)) {
      g_dbus_method_invocation_return_dbus_error(invocation,
        "app.copypaste.WaylandIntegration.Error.Unauthorized", "Unauthorized companion.");
      return;
    }
    cancel_bridge();
    bridge_transaction = transaction;
    bridge_sender = sender;
    bridge_begin = G_DBUS_METHOD_INVOCATION(g_object_ref(invocation));
    bridge_timeout = g_timeout_add_seconds(120, bridge_expired, nullptr);
    if (!launch_quick_paste(bridge_transaction)) cancel_bridge();
    return;
  }
  const bool selection = g_strcmp0(method, "SelectionCommitted") == 0;
  const bool cancelling = g_strcmp0(method, "CancelQuickPaste") == 0;
  const bool child = authorised_child(sender);
  const bool companion = bridge_accepted && bridge_sender == sender;
  if (bridge_transaction != transaction ||
      (selection && (!child || bridge_begin == nullptr || bridge_accepted)) ||
      (cancelling ? (!child && !companion) : (!selection && !companion))) {
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", FALSE));
    return;
  }
  if (g_strcmp0(method, "PasteIntoRestoredWindow") == 0) {
    const bool pasted = portal != nullptr && portal->send_ctrl_v();
    if (bridge_selection != nullptr) {
      g_dbus_method_invocation_return_value(bridge_selection, g_variant_new("(b)", pasted));
      g_object_unref(bridge_selection);
      bridge_selection = nullptr;
    }
    cancel_bridge(false, false);
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", pasted));
  } else if (cancelling) {
    // A child cancellation is copy-only: notify the companion so it restores
    // the previously focused window rather than treating it as a paste.
    cancel_bridge(child);
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", TRUE));
  } else if (selection) {
    bridge_selection = G_DBUS_METHOD_INVOCATION(g_object_ref(invocation));
    g_dbus_method_invocation_return_value(bridge_begin, g_variant_new("(b)", TRUE));
    g_object_unref(bridge_begin); bridge_begin = nullptr;
    bridge_accepted = true;
  } else {
    g_dbus_method_invocation_return_dbus_error(invocation,
      "app.copypaste.WaylandIntegration.Error.InvalidTransaction", "Unknown method.");
  }
}

const GDBusInterfaceVTable kBridgeVTable = {bridge_call, nullptr, nullptr, {nullptr}};

void bridge_bus_acquired(GDBusConnection* connection, const gchar*, gpointer) {
  static const gchar xml[] =
      "<node><interface name='app.copypaste.WaylandIntegration'>"
      "<method name='AwaitQuickPaste'><arg type='b' direction='out'/></method>"
      "<method name='BeginQuickPaste'><arg type='s' direction='in'/><arg type='b' direction='out'/></method>"
      "<method name='PasteIntoRestoredWindow'><arg type='s' direction='in'/><arg type='b' direction='out'/></method>"
      "<method name='CancelQuickPaste'><arg type='s' direction='in'/><arg type='b' direction='out'/></method>"
      "<method name='SelectionCommitted'><arg type='s' direction='in'/><arg type='b' direction='out'/></method>"
      "<method name='QuickPasteInputReady'><arg type='s' direction='in'/><arg type='b' direction='out'/></method>"
      "<method name='OpenMain'><arg type='s' direction='in'/><arg type='b' direction='out'/></method>"
      "<method name='OpenSettings'><arg type='s' direction='in'/><arg type='b' direction='out'/></method>"
      "<signal name='TransactionCancelled'><arg type='s'/></signal>"
      "</interface></node>";
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusNodeInfo) node = g_dbus_node_info_new_for_xml(xml, &error);
  if (node == nullptr) return;
  g_dbus_connection_register_object(connection, kBridgePath, node->interfaces[0],
                                    &kBridgeVTable, nullptr, nullptr, nullptr);
  bridge_connection = G_DBUS_CONNECTION(g_object_ref(connection));
  g_dbus_connection_signal_subscribe(
      connection, "org.freedesktop.DBus", "org.freedesktop.DBus", "NameOwnerChanged",
      "/org/freedesktop/DBus", nullptr, G_DBUS_SIGNAL_FLAGS_NONE,
      [](GDBusConnection*, const gchar*, const gchar*, const gchar*, const gchar*,
         GVariant* parameters, gpointer) {
        const gchar* name = nullptr;
        const gchar* old_owner = nullptr;
        const gchar* new_owner = nullptr;
        g_variant_get(parameters, "(&s&s&s)", &name, &old_owner, &new_owner);
        (void)new_owner;
        if ((g_strcmp0(name, "app.copypaste.GnomeIntegration") == 0 ||
             g_strcmp0(name, "org.kde.KWin") == 0) &&
            ((!bridge_sender.empty() &&
              g_strcmp0(old_owner, bridge_sender.c_str()) == 0) ||
             (!bridge_waiter_owner.empty() &&
              g_strcmp0(old_owner, bridge_waiter_owner.c_str()) == 0))) {
          cancel_bridge();
        }
      }, nullptr, nullptr);
}

FlMethodChannel* create_channel(FlBinaryMessenger* messenger, const char* name,
                                FlMethodChannelMethodCallHandler handler) {
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  FlMethodChannel* channel =
      fl_method_channel_new(messenger, name, FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, handler, nullptr, nullptr);
  return channel;
}

}  // namespace

void register_linux_host_channels(FlBinaryMessenger* messenger,
                                  GtkApplication* application) {
  // Main, Quick Paste, and protected presentation engines have independent
  // binary messengers. Keep a channel set alive for each instead of treating
  // the process as a singleton Flutter engine.
  if (state == nullptr) state = new ChannelState{application, {}};
  if (portal == nullptr) {
    GList* windows = gtk_application_get_windows(application);
    if (windows != nullptr) {
      portal = std::make_unique<LinuxPortal>(GTK_WINDOW(windows->data),
          LinuxPortalCallbacks{[](const std::string&, const std::string&) {
            wake_bridge_waiter();
          }, [] { cancel_bridge(); }});
      gnome_shortcuts = std::make_unique<LinuxGnomeShortcuts>([](const std::string&) {
        // Both native shortcut paths release the same authenticated waiter.
        wake_bridge_waiter();
      });
      if (g_object_get_data(G_OBJECT(application), "copypaste-quick-paste") == nullptr) {
        g_bus_own_name(G_BUS_TYPE_SESSION, kBridgeName, G_BUS_NAME_OWNER_FLAGS_NONE,
                       bridge_bus_acquired, nullptr, nullptr, nullptr, nullptr);
      }
    }
  }
  if (x11_quick_paste == nullptr) {
    x11_quick_paste = std::make_unique<LinuxX11QuickPaste>();
  }
  ChannelState* engine_state = new ChannelState{application, {}};
  engine_states.push_back(engine_state);
  engine_state->channels = {
      create_channel(messenger, kSecurity, security_call),
      create_channel(messenger, kPairing, pairing_call),
      create_channel(messenger, kQuickPasteHost, quick_paste_call),
      create_channel(messenger, kQuickPasteContext, quick_paste_context_call),
      create_channel(messenger, kShortcuts, shortcuts_call),
      create_channel(messenger, kIntegration, integration_call),
      create_channel(messenger, kUpdates, update_call),
      create_channel(messenger, kLifecycle, lifecycle_call),
      create_channel(messenger, kPairingLinks, pairing_links_call),
  };
  if (g_object_get_data(G_OBJECT(application), "copypaste-quick-paste") != nullptr) {
    g_bus_watch_name(
        G_BUS_TYPE_SESSION, kBridgeName, G_BUS_NAME_WATCHER_FLAGS_NONE,
        nullptr,
        [](GDBusConnection*, const gchar*, gpointer) { quit_context_when_idle(); },
        nullptr, nullptr);
  }
}

void deliver_linux_pairing_uri(const gchar* uri) {
  if (uri == nullptr || *uri == '\0') return;
  if (state == nullptr || state->application == nullptr || engine_states.empty() ||
      engine_states.front()->channels[8] == nullptr) {
    if (state != nullptr && state->application != nullptr) {
      g_object_set_data_full(G_OBJECT(state->application), "copypaste-pairing-uri",
                             g_strdup(uri), g_free);
    }
    return;
  }
  FlValue* arguments = fl_value_new_string(uri);
  fl_method_channel_invoke_method(engine_states.front()->channels[8], "openPairingUri",
                                  arguments, nullptr, nullptr, nullptr);
  fl_value_unref(arguments);
}
