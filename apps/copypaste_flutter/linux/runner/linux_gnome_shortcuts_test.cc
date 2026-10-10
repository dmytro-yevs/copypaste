// Private-bus GIO fixture for the GNOME 46/47 shortcut fallback. It validates
// the D-Bus transport on any host with GIO and dbus-daemon; it is not evidence
// that Mutter's real shortcut grab works on Linux.

#include "linux_gnome_shortcuts.h"

#include <gio/gio.h>

#include <functional>
#include <memory>
#include <string>

namespace {

constexpr char kName[] = "app.copypaste.GnomeIntegration";
constexpr char kPath[] = "/app/copypaste/GnomeShortcuts";
constexpr char kInterface[] = "app.copypaste.GnomeShortcuts";

const gchar kXml[] =
    "<node><interface name='app.copypaste.GnomeShortcuts'>"
    "<method name='Version'><arg type='u' direction='out'/></method>"
    "<method name='RegisterShortcut'><arg type='s' direction='in'/>"
    "<arg type='s' direction='in'/><arg type='b' direction='out'/>"
    "<arg type='s' direction='out'/></method>"
    "<method name='UnregisterShortcut'><arg type='s' direction='in'/>"
    "<arg type='b' direction='out'/></method>"
    "<signal name='Activated'><arg type='s'/></signal>"
    "</interface></node>";

bool wait_until(const std::function<bool()>& ready) {
  const gint64 deadline = g_get_monotonic_time() + G_TIME_SPAN_SECOND;
  while (!ready() && g_get_monotonic_time() < deadline) {
    g_main_context_iteration(nullptr, false);
    g_usleep(1000);
  }
  return ready();
}

class CompanionFixture {
 public:
  CompanionFixture() {
    bus_ = g_test_dbus_new(G_TEST_DBUS_NONE);
    g_test_dbus_up(bus_);
    g_autoptr(GError) error = nullptr;
    connection_ = g_dbus_connection_new_for_address_sync(
        g_test_dbus_get_bus_address(bus_),
        static_cast<GDBusConnectionFlags>(G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
                                          G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION),
        nullptr, nullptr, &error);
    g_assert_no_error(error);
    g_assert_nonnull(connection_);
    g_autoptr(GDBusNodeInfo) node = g_dbus_node_info_new_for_xml(kXml, &error);
    g_assert_no_error(error);
    registration_ = g_dbus_connection_register_object(
        connection_, kPath, node->interfaces[0], &kVTable, this, nullptr, &error);
    g_assert_no_error(error);
    g_assert_cmpuint(registration_, !=, 0);
    name_owner_ = g_bus_own_name_on_connection(
        connection_, kName, G_BUS_NAME_OWNER_FLAGS_NONE, nullptr, nullptr, nullptr,
        nullptr);
    g_assert_cmpuint(name_owner_, !=, 0);
    g_assert_true(wait_until([this] {
      g_autoptr(GError) owner_error = nullptr;
      g_autoptr(GVariant) reply = g_dbus_connection_call_sync(
          connection_, "org.freedesktop.DBus", "/org/freedesktop/DBus",
          "org.freedesktop.DBus", "NameHasOwner", g_variant_new("(s)", kName),
          G_VARIANT_TYPE("(b)"), G_DBUS_CALL_FLAGS_NONE, 200, nullptr, &owner_error);
      gboolean has_owner = FALSE;
      if (reply != nullptr) g_variant_get(reply, "(b)", &has_owner);
      return has_owner;
    }));
  }

  ~CompanionFixture() {
    if (name_owner_ != 0) g_bus_unown_name(name_owner_);
    if (registration_ != 0) g_dbus_connection_unregister_object(connection_, registration_);
    g_clear_object(&connection_);
    g_test_dbus_down(bus_);
    g_clear_object(&bus_);
  }

  void emit_activated(const char* id) {
    g_autoptr(GError) error = nullptr;
    g_assert_true(g_dbus_connection_emit_signal(connection_, nullptr, kPath, kInterface,
                                                 "Activated", g_variant_new("(s)", id),
                                                 &error));
    g_assert_no_error(error);
  }

  void drop_name() {
    g_bus_unown_name(name_owner_);
    name_owner_ = 0;
  }

  const std::string& accelerator() const { return accelerator_; }

 private:
  static void handle_call(GDBusConnection*, const gchar*, const gchar*,
                          const gchar*, const gchar* method, GVariant* parameters,
                          GDBusMethodInvocation* invocation, gpointer data) {
    auto* fixture = static_cast<CompanionFixture*>(data);
    if (g_strcmp0(method, "Version") == 0) {
      g_dbus_method_invocation_return_value(invocation, g_variant_new("(u)", 1U));
      return;
    }
    if (g_strcmp0(method, "RegisterShortcut") == 0) {
      const gchar* id = nullptr;
      const gchar* accelerator = nullptr;
      g_variant_get(parameters, "(&s&s)", &id, &accelerator);
      fixture->accelerator_ = accelerator == nullptr ? "" : accelerator;
      g_dbus_method_invocation_return_value(
          invocation, g_variant_new("(bs)", id != nullptr && *id != '\0', "Ctrl+Shift+C"));
      return;
    }
    g_assert_cmpstr(method, ==, "UnregisterShortcut");
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", TRUE));
  }

  static const GDBusInterfaceVTable kVTable;
  GTestDBus* bus_ = nullptr;
  GDBusConnection* connection_ = nullptr;
  guint registration_ = 0;
  guint name_owner_ = 0;
  std::string accelerator_;
};

const GDBusInterfaceVTable CompanionFixture::kVTable = {handle_call, nullptr, nullptr,
                                                         {nullptr}};

void test_register_activate_and_owner_loss() {
  CompanionFixture fixture;
  guint activations = 0;
  LinuxGnomeShortcuts shortcuts([&activations](const std::string& id) {
    if (id == "copypaste.quick-paste") ++activations;
  });
  g_assert_true(wait_until([&shortcuts] { return shortcuts.is_available(); }));

  ShortcutRequest request;
  request.id = "copypaste.quick-paste";
  request.usage = std::to_string(0x70006U);
  request.modifiers = {"control", "shift"};
  // The portal-style hint is intentionally ignored by the GNOME adapter.
  request.preferred_trigger = "CTRL+SHIFT+C";
  bool complete = false;
  ShortcutResult result;
  shortcuts.register_shortcut(request, [&complete, &result](ShortcutResult response) {
    complete = true;
    result = std::move(response);
  });
  g_assert_true(wait_until([&complete] { return complete; }));
  g_assert_true(result.registered);
  g_assert_cmpstr(result.trigger_description.c_str(), ==, "Ctrl+Shift+C");
  g_assert_cmpstr(fixture.accelerator().c_str(), ==, "<Control><Shift>c");

  fixture.emit_activated(request.id.c_str());
  g_assert_true(wait_until([&activations] { return activations == 1; }));
  fixture.drop_name();
  g_assert_true(wait_until([&shortcuts] {
    return !shortcuts.is_available() && !shortcuts.shortcut_registered();
  }));
}

}  // namespace

int main(int argc, char** argv) {
  g_test_init(&argc, &argv, nullptr);
  g_test_add_func("/linux_gnome_shortcuts/register_activate_and_owner_loss",
                  test_register_activate_and_owner_loss);
  return g_test_run();
}
