// Standalone private-bus contract fixture for linux_packagekit.cc.
// It is compiled by scripts/test-linux-packagekit.sh inside a disposable Linux
// root, where /usr/lib/copypaste provenance can be created safely.
#define private public
#include "../linux_packagekit.cc"
#undef private

#include <cassert>

namespace {

struct Service {
  std::pair<gboolean, gboolean> flags{FALSE, FALSE};
  GMainContext* context = nullptr;
  GMainLoop* loop = nullptr;
  GMutex mutex;
  GCond ready;
  gboolean started = FALSE;
};

GVariant* property_value(const gchar* property, gboolean wrong_type,
                         gboolean missing_install_files) {
  if (g_strcmp0(property, "BackendName") == 0) {
    return g_variant_new_variant(wrong_type ? g_variant_new_uint64(1)
                                            : g_variant_new_string("apt"));
  }
  if (g_strcmp0(property, "Roles") == 0) {
    return g_variant_new_variant(wrong_type ? g_variant_new_string("install-files")
        : g_variant_new_uint64(missing_install_files ? 0 : (G_GUINT64_CONSTANT(1) << 10)));
  }
  return nullptr;
}

void handle_properties(GDBusConnection*, const gchar*, const gchar*,
                       const gchar*, const gchar* method, GVariant* params,
                       GDBusMethodInvocation* invocation, gpointer data) {
  auto* flags = static_cast<std::pair<gboolean, gboolean>*>(data);
  if (g_strcmp0(method, "Get") != 0) {
    g_dbus_method_invocation_return_dbus_error(invocation,
        "org.freedesktop.DBus.Error.UnknownMethod", "unexpected method");
    return;
  }
  const gchar* interface_name = nullptr;
  const gchar* property = nullptr;
  g_variant_get(params, "(&s&s)", &interface_name, &property);
  assert(g_strcmp0(interface_name, kPackageKitInterface) == 0);
  GVariant* value = property_value(property, flags->first, flags->second);
  if (value == nullptr) {
    g_dbus_method_invocation_return_dbus_error(invocation,
        "org.freedesktop.DBus.Error.InvalidArgs", "unknown property");
    return;
  }
  g_dbus_method_invocation_return_value(invocation, g_variant_new("(@v)", value));
}

const GDBusInterfaceVTable kPropertiesVTable = {handle_properties, nullptr, nullptr, {nullptr}};

gpointer service_thread(gpointer data) {
  auto* service = static_cast<Service*>(data);
  service->context = g_main_context_new();
  g_main_context_push_thread_default(service->context);
  service->loop = g_main_loop_new(service->context, FALSE);
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) connection = g_bus_get_sync(G_BUS_TYPE_SYSTEM, nullptr, &error);
  assert(connection != nullptr);
  g_autoptr(GVariant) request_name = g_dbus_connection_call_sync(
      connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
      "org.freedesktop.DBus", "RequestName",
      g_variant_new("(su)", kPackageKitService, 0), G_VARIANT_TYPE("(u)"),
      G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
  assert(request_name != nullptr);
  static const gchar xml[] =
      "<node><interface name='org.freedesktop.DBus.Properties'>"
      "<method name='Get'><arg type='s' direction='in'/><arg type='s' direction='in'/>"
      "<arg type='v' direction='out'/></method></interface></node>";
  g_autoptr(GDBusNodeInfo) node = g_dbus_node_info_new_for_xml(xml, &error);
  assert(node != nullptr);
  const guint registration = g_dbus_connection_register_object(
      connection, kPackageKitPath, node->interfaces[0], &kPropertiesVTable, &service->flags,
      nullptr, &error);
  assert(registration != 0);
  g_mutex_lock(&service->mutex);
  service->started = TRUE;
  g_cond_signal(&service->ready);
  g_mutex_unlock(&service->mutex);
  g_main_loop_run(service->loop);
  g_dbus_connection_unregister_object(connection, registration);
  g_main_loop_unref(service->loop);
  g_main_context_pop_thread_default(service->context);
  g_main_context_unref(service->context);
  return nullptr;
}

bool run_case(gboolean wrong_type, gboolean missing_install_files) {
  Service service;
  g_mutex_init(&service.mutex);
  g_cond_init(&service.ready);
  service.flags = {wrong_type, missing_install_files};
  GThread* thread = g_thread_new("packagekit-fixture", service_thread, &service);
  g_mutex_lock(&service.mutex);
  while (!service.started) g_cond_wait(&service.ready, &service.mutex);
  g_mutex_unlock(&service.mutex);
  std::string reason;
  const bool result = packagekit_available(&reason);
  g_main_context_invoke(service.context, [](gpointer data) {
    g_main_loop_quit(static_cast<Service*>(data)->loop);
    return G_SOURCE_REMOVE;
  }, &service);
  g_thread_join(thread);
  g_cond_clear(&service.ready);
  g_mutex_clear(&service.mutex);
  return result;
}

}  // namespace

int main() {
  // The harness owns org.freedesktop.PackageKit on its private system bus.
  // Successful typed BackendName + Roles(t=1024) is accepted; absent role and
  // wrong variants are rejected before an install transaction can start.
  assert(run_case(FALSE, FALSE));
  assert(!run_case(FALSE, TRUE));
  assert(!run_case(TRUE, FALSE));
  return 0;
}
