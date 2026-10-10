// Standalone private-bus lifecycle fixture for linux_packagekit.cc.
// Include production code so its private request callbacks are exercised.
#include "../linux_packagekit.cc"

#include <cassert>
#include <functional>
#include <string>

namespace {
constexpr char kTransactionPath[] = "/org/freedesktop/PackageKit/transactions/copypaste";
enum class ReplyMode { kFinishedBeforeReply, kHoldReply };

struct Options {
  gboolean wrong_type = FALSE;
  gboolean missing_install_files = FALSE;
  ReplyMode reply_mode = ReplyMode::kFinishedBeforeReply;
};

struct Service {
  explicit Service(Options options) : options(options) {
    g_mutex_init(&mutex);
    g_cond_init(&condition);
  }
  ~Service() {
    g_cond_clear(&condition);
    g_mutex_clear(&mutex);
  }
  Options options;
  GMainContext* context = nullptr;
  GMainLoop* loop = nullptr;
  GMutex mutex;
  GCond condition;
  gboolean started = FALSE;
  gboolean install_seen = FALSE;
  gboolean late_reply_sent = FALSE;
  guint install_calls = 0;
  guint finished_signals = 0;
  GDBusMethodInvocation* held_invocation = nullptr;
};

struct Completion {
  guint calls = 0;
  LinuxPackageInstallResult result = LinuxPackageInstallResult::kFailed;
  std::string message;
};

GVariant* property_value(const Options& options, const gchar* property) {
  if (g_strcmp0(property, "BackendName") == 0) {
    return g_variant_new_variant(options.wrong_type ? g_variant_new_uint64(1)
                                                    : g_variant_new_string("apt"));
  }
  if (g_strcmp0(property, "Roles") == 0) {
    return g_variant_new_variant(
        options.wrong_type ? g_variant_new_string("install-files")
        : g_variant_new_uint64(options.missing_install_files
            ? 0 : (G_GUINT64_CONSTANT(1) << 10)));
  }
  return nullptr;
}

void emit_finished(Service* service, GDBusConnection* connection) {
  g_autoptr(GError) error = nullptr;
  assert(g_dbus_connection_emit_signal(
      connection, nullptr, kTransactionPath, kTransactionInterface, "Finished",
      g_variant_new("(uu)", kPackageKitExitSuccess, 0), &error));
  assert(error == nullptr);
  g_mutex_lock(&service->mutex);
  ++service->finished_signals;
  g_mutex_unlock(&service->mutex);
}

void handle_method(GDBusConnection* connection, const gchar*, const gchar*,
                   const gchar* interface_name, const gchar* method,
                   GVariant* parameters, GDBusMethodInvocation* invocation,
                   gpointer user_data) {
  auto* service = static_cast<Service*>(user_data);
  if (g_strcmp0(interface_name, "org.freedesktop.DBus.Properties") == 0) {
    if (g_strcmp0(method, "Get") != 0) {
      g_dbus_method_invocation_return_dbus_error(
          invocation, "org.freedesktop.DBus.Error.UnknownMethod", "unexpected method");
      return;
    }
    const gchar* requested_interface = nullptr;
    const gchar* property = nullptr;
    g_variant_get(parameters, "(&s&s)", &requested_interface, &property);
    assert(g_strcmp0(requested_interface, kPackageKitInterface) == 0);
    GVariant* value = property_value(service->options, property);
    if (value == nullptr) {
      g_dbus_method_invocation_return_dbus_error(
          invocation, "org.freedesktop.DBus.Error.InvalidArgs", "unknown property");
      return;
    }
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(@v)", value));
    return;
  }
  if (g_strcmp0(interface_name, kPackageKitInterface) == 0 &&
      g_strcmp0(method, "CreateTransaction") == 0) {
    g_dbus_method_invocation_return_value(invocation,
                                          g_variant_new("(o)", kTransactionPath));
    return;
  }
  if (g_strcmp0(interface_name, kTransactionInterface) == 0 &&
      g_strcmp0(method, "InstallFiles") == 0) {
    g_mutex_lock(&service->mutex);
    service->install_seen = TRUE;
    ++service->install_calls;
    g_cond_broadcast(&service->condition);
    const ReplyMode reply_mode = service->options.reply_mode;
    if (reply_mode == ReplyMode::kHoldReply) {
      service->held_invocation =
          G_DBUS_METHOD_INVOCATION(g_object_ref(invocation));
    }
    g_mutex_unlock(&service->mutex);
    if (reply_mode == ReplyMode::kHoldReply) return;

    // These duplicate terminal signals are queued before the method reply.
    emit_finished(service, connection);
    emit_finished(service, connection);
    g_dbus_method_invocation_return_value(invocation, g_variant_new("()"));
    return;
  }
  g_dbus_method_invocation_return_dbus_error(
      invocation, "org.freedesktop.DBus.Error.UnknownMethod", "unexpected method");
}

const GDBusInterfaceVTable kServiceVTable = {handle_method, nullptr, nullptr, {nullptr}};

gpointer service_thread(gpointer data) {
  auto* service = static_cast<Service*>(data);
  service->context = g_main_context_new();
  g_main_context_push_thread_default(service->context);
  service->loop = g_main_loop_new(service->context, FALSE);
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) connection =
      g_bus_get_sync(G_BUS_TYPE_SYSTEM, nullptr, &error);
  assert(connection != nullptr);
  g_autoptr(GVariant) name = g_dbus_connection_call_sync(
      connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
      "org.freedesktop.DBus", "RequestName",
      g_variant_new("(su)", kPackageKitService, 0), G_VARIANT_TYPE("(u)"),
      G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
  assert(name != nullptr);
  static const gchar xml[] =
      "<node><interface name='org.freedesktop.DBus.Properties'>"
      "<method name='Get'><arg type='s' direction='in'/><arg type='s' direction='in'/>"
      "<arg type='v' direction='out'/></method></interface>"
      "<interface name='org.freedesktop.PackageKit'>"
      "<method name='CreateTransaction'><arg type='o' direction='out'/></method></interface>"
      "<interface name='org.freedesktop.PackageKit.Transaction'>"
      "<method name='InstallFiles'><arg type='t' direction='in'/><arg type='as' direction='in'/>"
      "</method><signal name='Finished'><arg type='u'/><arg type='u'/></signal>"
      "</interface></node>";
  g_autoptr(GDBusNodeInfo) node = g_dbus_node_info_new_for_xml(xml, &error);
  assert(node != nullptr);
  const guint properties = g_dbus_connection_register_object(
      connection, kPackageKitPath, node->interfaces[0], &kServiceVTable, service, nullptr, &error);
  const guint packagekit = g_dbus_connection_register_object(
      connection, kPackageKitPath, node->interfaces[1], &kServiceVTable, service, nullptr, &error);
  const guint transaction = g_dbus_connection_register_object(
      connection, kTransactionPath, node->interfaces[2], &kServiceVTable, service, nullptr, &error);
  assert(properties != 0 && packagekit != 0 && transaction != 0);
  g_mutex_lock(&service->mutex);
  service->started = TRUE;
  g_cond_broadcast(&service->condition);
  g_mutex_unlock(&service->mutex);
  g_main_loop_run(service->loop);
  g_mutex_lock(&service->mutex);
  g_clear_object(&service->held_invocation);
  g_mutex_unlock(&service->mutex);
  g_dbus_connection_unregister_object(connection, transaction);
  g_dbus_connection_unregister_object(connection, packagekit);
  g_dbus_connection_unregister_object(connection, properties);
  g_main_loop_unref(service->loop);
  g_main_context_pop_thread_default(service->context);
  g_main_context_unref(service->context);
  return nullptr;
}

class ServiceRunner {
 public:
  explicit ServiceRunner(Options options) : service_(options) {
    thread_ = g_thread_new("packagekit-fixture", service_thread, &service_);
    g_mutex_lock(&service_.mutex);
    while (!service_.started) g_cond_wait(&service_.condition, &service_.mutex);
    g_mutex_unlock(&service_.mutex);
  }
  ~ServiceRunner() {
    g_main_context_invoke(service_.context, [](gpointer data) {
      g_main_loop_quit(static_cast<Service*>(data)->loop);
      return G_SOURCE_REMOVE;
    }, &service_);
    g_thread_join(thread_);
  }
  Service* get() { return &service_; }
 private:
  Service service_;
  GThread* thread_ = nullptr;
};

bool install_seen(Service* service) {
  g_mutex_lock(&service->mutex);
  const bool result = service->install_seen;
  g_mutex_unlock(&service->mutex);
  return result;
}

void reply_late(Service* service) {
  g_main_context_invoke(service->context, [](gpointer data) {
    auto* service = static_cast<Service*>(data);
    g_mutex_lock(&service->mutex);
    GDBusMethodInvocation* invocation = service->held_invocation;
    service->held_invocation = nullptr;
    service->late_reply_sent = TRUE;
    g_cond_broadcast(&service->condition);
    g_mutex_unlock(&service->mutex);
    assert(invocation != nullptr);
    g_dbus_method_invocation_return_value(invocation, g_variant_new("()"));
    g_object_unref(invocation);
    return G_SOURCE_REMOVE;
  }, service);
}

bool late_reply_sent(Service* service) {
  g_mutex_lock(&service->mutex);
  const bool result = service->late_reply_sent;
  g_mutex_unlock(&service->mutex);
  return result;
}

void drain_context(gint64 milliseconds) {
  const gint64 deadline = g_get_monotonic_time() + milliseconds * 1000;
  while (g_get_monotonic_time() < deadline) {
    while (g_main_context_pending(nullptr)) g_main_context_iteration(nullptr, FALSE);
    g_usleep(1000);
  }
}

void wait_until(const std::function<bool()>& condition) {
  const gint64 deadline = g_get_monotonic_time() + 3 * G_USEC_PER_SEC;
  while (!condition()) {
    assert(g_get_monotonic_time() < deadline);
    while (g_main_context_pending(nullptr)) g_main_context_iteration(nullptr, FALSE);
    g_usleep(1000);
  }
  while (g_main_context_pending(nullptr)) g_main_context_iteration(nullptr, FALSE);
}

InstallRequest* new_request(GDBusConnection* connection, Completion* completion,
                            std::string* staging_directory) {
  g_autoptr(GError) error = nullptr;
  g_autofree gchar* directory = g_dir_make_tmp("copypaste-packagekit-test-XXXXXX", &error);
  assert(directory != nullptr);
  const std::string contents = "fixture package";
  g_autofree gchar* path = g_build_filename(directory, "copypaste-linux-x86_64.deb", nullptr);
  assert(g_file_set_contents(path, contents.c_str(), contents.size(), &error));
  g_autofree gchar* sha256 = g_compute_checksum_for_string(
      G_CHECKSUM_SHA256, contents.c_str(), static_cast<gssize>(contents.size()));
  auto* request = new InstallRequest;
  request->connection = G_DBUS_CONNECTION(g_object_ref(connection));
  request->package_path = path;
  request->staging_directory = directory;
  request->expected_sha256 = sha256;
  assert(hash_regular_file(request->package_path, request->expected_sha256, &request->identity));
  request->done = [completion](LinuxPackageInstallResult result, std::string message) {
    ++completion->calls;
    completion->result = result;
    completion->message = std::move(message);
  };
  *staging_directory = directory;
  return request;
}

void start_transaction(InstallRequest* request) {
  g_dbus_connection_call(request->connection, kPackageKitService, kPackageKitPath,
                         kPackageKitInterface, "CreateTransaction", nullptr,
                         G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, -1,
                         request->cancellable, on_create_transaction_ready,
                         (retain(request), request));
}

void test_properties() {
  for (const auto& item : {std::pair<Options, bool>{{FALSE, FALSE}, true},
                           {{FALSE, TRUE}, false}, {{TRUE, FALSE}, false}}) {
    ServiceRunner service(item.first);
    std::string reason;
    assert(packagekit_available(&reason) == item.second);
  }
}

void test_finished_before_reply() {
  ServiceRunner service({FALSE, FALSE, ReplyMode::kFinishedBeforeReply});
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) connection = g_bus_get_sync(G_BUS_TYPE_SYSTEM, nullptr, &error);
  assert(connection != nullptr);
  Completion completion;
  std::string directory;
  start_transaction(new_request(connection, &completion, &directory));
  wait_until([&completion] { return completion.calls == 1; });
  assert(completion.result == LinuxPackageInstallResult::kRestartRequired);
  assert(completion.message == "The package update completed.");
  assert(!g_file_test(directory.c_str(), G_FILE_TEST_EXISTS));
  g_mutex_lock(&service.get()->mutex);
  assert(service.get()->install_calls == 1 && service.get()->finished_signals == 2);
  g_mutex_unlock(&service.get()->mutex);
  drain_context(50);
  assert(completion.calls == 1);
}

void test_timeout_cancels_late_reply() {
  ServiceRunner service({FALSE, FALSE, ReplyMode::kHoldReply});
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) connection = g_bus_get_sync(G_BUS_TYPE_SYSTEM, nullptr, &error);
  assert(connection != nullptr);
  Completion completion;
  std::string directory;
  InstallRequest* request = new_request(connection, &completion, &directory);
  start_transaction(request);
  wait_until([&service] { return install_seen(service.get()); });
  // Exercise the production watchdog without waiting 90 seconds.
  assert(terminal_timeout(request) == G_SOURCE_REMOVE);
  assert(completion.calls == 1);
  assert(completion.message == "PackageKit did not reach a terminal state.");
  assert(!g_file_test(directory.c_str(), G_FILE_TEST_EXISTS));
  reply_late(service.get());
  wait_until([&service] { return late_reply_sent(service.get()); });
  drain_context(50);
  assert(completion.calls == 1);
}
}  // namespace

int main() {
  test_properties();
  test_finished_before_reply();
  test_timeout_cancels_late_reply();
  return 0;
}
