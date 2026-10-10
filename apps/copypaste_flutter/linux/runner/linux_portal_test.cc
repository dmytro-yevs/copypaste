// Run scripts/test-linux-portal.sh to compile and execute this fixture.

#include <gio/gio.h>
#include <glib.h>

#include <chrono>
#include <condition_variable>
#include <functional>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

// The runner has no library target. Include the implementation so this test
// exercises the private portal request lifecycle with a real session bus.
#define private public
#include "linux_portal.h"
#undef private
#include "linux_portal.cc"

namespace {

constexpr char kTestMethod[] = "Test";

enum class ReplyMode {
  kEarlyResponse,
  kError,
  kPending,
  kQueuedResponse,
  kKeyDownTimeout,
};

class PortalFixture {
 public:
  PortalFixture() {
    bus_ = g_test_dbus_new(G_TEST_DBUS_NONE);
    g_test_dbus_up(bus_);
    bus_address_ = g_test_dbus_get_bus_address(bus_);
    server_context_ = g_main_context_new();
    server_thread_ = std::thread([this] { run_server(); });

    std::unique_lock<std::mutex> lock(mutex_);
    const bool started = ready_.wait_for(
        lock, std::chrono::seconds(5), [this] { return server_ready_ || server_failed_; });
    g_assert_true(started);
    g_assert_false(server_failed_);
  }

  ~PortalFixture() {
    bool server_ready = false;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      server_ready = server_ready_;
    }
    if (server_ready) {
      g_main_context_invoke_full(
          server_context_, G_PRIORITY_DEFAULT,
          [](gpointer data) {
            auto* fixture = static_cast<PortalFixture*>(data);
            g_main_loop_quit(fixture->server_loop_);
            return G_SOURCE_REMOVE;
          },
          this, nullptr);
    }
    server_thread_.join();
    g_main_context_unref(server_context_);
    g_clear_object(&server_connection_);
    g_test_dbus_down(bus_);
    g_clear_object(&bus_);
  }

  std::shared_ptr<LinuxPortal::Impl> new_client() {
    return std::make_shared<LinuxPortal::Impl>(nullptr, LinuxPortalCallbacks{});
  }

  void set_mode(ReplyMode mode) {
    std::lock_guard<std::mutex> lock(mutex_);
    mode_ = mode;
  }

  bool wait_for_calls(guint expected) {
    std::unique_lock<std::mutex> lock(mutex_);
    return calls_ready_.wait_for(
        lock, std::chrono::seconds(1), [this, expected] { return calls_ >= expected; });
  }

  bool wait_for_key(gint keycode, guint state) {
    std::unique_lock<std::mutex> lock(mutex_);
    return keys_ready_.wait_for(lock, std::chrono::seconds(1),
                                [this, keycode, state] {
      return std::find(key_events_.begin(), key_events_.end(),
                       std::make_pair(keycode, state)) != key_events_.end();
    });
  }

  guint calls() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return calls_;
  }

  bool emit_session_closed(const std::string& session_path) {
    GDBusConnection* server = nullptr;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      if (server_connection_ != nullptr) {
        server = G_DBUS_CONNECTION(g_object_ref(server_connection_));
      }
    }
    if (server == nullptr) return false;
    g_autoptr(GError) error = nullptr;
    const gboolean emitted = g_dbus_connection_emit_signal(
        server, nullptr, session_path.c_str(), kSession, "Closed", nullptr, &error);
    const gboolean flushed = emitted && g_dbus_connection_flush_sync(
        server, nullptr, &error);
    g_object_unref(server);
    return flushed && error == nullptr;
  }

  bool emit_shortcut_activated(const std::string& session_path,
                               const std::string& shortcut_id,
                               const std::string& activation_token) {
    GDBusConnection* server = nullptr;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      if (server_connection_ != nullptr) {
        server = G_DBUS_CONNECTION(g_object_ref(server_connection_));
      }
    }
    if (server == nullptr) return false;
    GVariantBuilder options;
    g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
    option_string(&options, "activation_token", activation_token);
    g_autoptr(GError) error = nullptr;
    const gboolean emitted = g_dbus_connection_emit_signal(
        server, nullptr, kPortalPath, kGlobalShortcuts, "Activated",
        g_variant_new("(ost@a{sv})", session_path.c_str(), shortcut_id.c_str(),
                      static_cast<guint64>(1), g_variant_builder_end(&options)),
        &error);
    const gboolean flushed = emitted && g_dbus_connection_flush_sync(
        server, nullptr, &error);
    g_object_unref(server);
    return flushed && error == nullptr;
  }

  bool saw_sequence(const std::vector<std::string>& expected) const {
    std::lock_guard<std::mutex> lock(mutex_);
    size_t expected_index = 0;
    for (const std::string& call : method_calls_) {
      if (expected_index < expected.size() &&
          call == expected[expected_index]) {
        ++expected_index;
      }
    }
    return expected_index == expected.size();
  }

  void set_keyboard_granted(bool granted) {
    std::lock_guard<std::mutex> lock(mutex_);
    keyboard_granted_ = granted;
  }

 private:
  void run_server() {
    g_main_context_push_thread_default(server_context_);
    g_autoptr(GError) error = nullptr;
    g_autoptr(GDBusConnection) server = g_dbus_connection_new_for_address_sync(
        bus_address_.c_str(),
        static_cast<GDBusConnectionFlags>(
            G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
            G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION),
        nullptr, nullptr, &error);
    if (server == nullptr) return report_start_failure(error);

    static constexpr char kIntrospection[] = R"xml(
      <node>
        <interface name='org.freedesktop.portal.RemoteDesktop'>
          <method name='Test'>
            <arg type='a{sv}' direction='in'/>
            <arg type='o' direction='out'/>
          </method>
          <method name='NotifyKeyboardKeycode'>
            <arg type='o' direction='in'/>
            <arg type='a{sv}' direction='in'/>
            <arg type='i' direction='in'/>
            <arg type='u' direction='in'/>
          </method>
          <method name='CreateSession'>
            <arg type='a{sv}' direction='in'/>
            <arg type='o' direction='out'/>
          </method>
          <method name='SelectDevices'>
            <arg type='o' direction='in'/>
            <arg type='a{sv}' direction='in'/>
            <arg type='o' direction='out'/>
          </method>
          <method name='Start'>
            <arg type='o' direction='in'/>
            <arg type='s' direction='in'/>
            <arg type='a{sv}' direction='in'/>
            <arg type='o' direction='out'/>
          </method>
        </interface>
        <interface name='org.freedesktop.portal.GlobalShortcuts'>
          <method name='CreateSession'>
            <arg type='a{sv}' direction='in'/>
            <arg type='o' direction='out'/>
          </method>
          <method name='BindShortcuts'>
            <arg type='o' direction='in'/>
            <arg type='a(sa{sv})' direction='in'/>
            <arg type='s' direction='in'/>
            <arg type='a{sv}' direction='in'/>
            <arg type='o' direction='out'/>
          </method>
          <signal name='Activated'>
            <arg type='o'/>
            <arg type='s'/>
            <arg type='t'/>
            <arg type='a{sv}'/>
          </signal>
        </interface>
        <interface name='org.freedesktop.DBus.Properties'>
          <method name='Get'>
            <arg type='s' direction='in'/>
            <arg type='s' direction='in'/>
            <arg type='v' direction='out'/>
          </method>
        </interface>
      </node>)xml";
    g_autoptr(GDBusNodeInfo) node = g_dbus_node_info_new_for_xml(kIntrospection, &error);
    if (node == nullptr) return report_start_failure(error);
    static const GDBusInterfaceVTable kVTable = {&PortalFixture::on_method_call,
                                                  nullptr, nullptr, {nullptr}};
    const guint remote_registration = g_dbus_connection_register_object(
        server, kPortalPath, node->interfaces[0], &kVTable, this, nullptr, &error);
    if (remote_registration == 0) return report_start_failure(error);
    const guint shortcuts_registration = g_dbus_connection_register_object(
        server, kPortalPath, node->interfaces[1], &kVTable, this, nullptr, &error);
    if (shortcuts_registration == 0) {
      g_dbus_connection_unregister_object(server, remote_registration);
      return report_start_failure(error);
    }
    const guint properties_registration = g_dbus_connection_register_object(
        server, kPortalPath, node->interfaces[2], &kVTable, this, nullptr, &error);
    if (properties_registration == 0) {
      g_dbus_connection_unregister_object(server, shortcuts_registration);
      g_dbus_connection_unregister_object(server, remote_registration);
      return report_start_failure(error);
    }
    g_autoptr(GVariant) name_reply = g_dbus_connection_call_sync(
        server, "org.freedesktop.DBus", "/org/freedesktop/DBus",
        "org.freedesktop.DBus", "RequestName",
        g_variant_new("(su)", kPortalName, 0), G_VARIANT_TYPE("(u)"),
        G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
    if (name_reply == nullptr) {
      g_dbus_connection_unregister_object(server, properties_registration);
      g_dbus_connection_unregister_object(server, shortcuts_registration);
      g_dbus_connection_unregister_object(server, remote_registration);
      return report_start_failure(error);
    }

    server_loop_ = g_main_loop_new(server_context_, false);
    {
      std::lock_guard<std::mutex> lock(mutex_);
      server_connection_ = G_DBUS_CONNECTION(g_object_ref(server));
      server_ready_ = true;
    }
    ready_.notify_all();
    g_main_loop_run(server_loop_);
    g_main_loop_unref(server_loop_);
    server_loop_ = nullptr;
    g_dbus_connection_unregister_object(server, properties_registration);
    g_dbus_connection_unregister_object(server, shortcuts_registration);
    g_dbus_connection_unregister_object(server, remote_registration);
    g_main_context_pop_thread_default(server_context_);
  }

  void report_start_failure(const GError* error) {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      server_failed_ = true;
      if (error != nullptr) server_error_ = error->message;
    }
    ready_.notify_all();
    g_main_context_pop_thread_default(server_context_);
  }

  static std::string request_path(GDBusMethodInvocation* invocation,
                                  GVariant* parameters) {
    std::string handle_token;
    const gsize children = g_variant_n_children(parameters);
    for (gsize index = 0; index < children; ++index) {
      g_autoptr(GVariant) child = g_variant_get_child_value(parameters, index);
      if (g_variant_is_of_type(child, G_VARIANT_TYPE_VARDICT)) {
        handle_token = variant_string(child, "handle_token");
        if (!handle_token.empty()) break;
      }
    }
    const gchar* sender = g_dbus_method_invocation_get_sender(invocation);
    if (sender == nullptr || *sender != ':' || handle_token.empty()) return {};
    std::string sender_path(sender + 1);
    for (char& character : sender_path) {
      if (!g_ascii_isalnum(character)) character = '_';
    }
    return std::string(kPortalPath) + "/request/" + sender_path + "/" +
           handle_token;
  }

  static GVariant* session_result(const std::string& session_path) {
    GVariantBuilder results;
    g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
    g_variant_builder_add(&results, "{sv}", "session_handle",
                          g_variant_new_object_path(session_path.c_str()));
    return g_variant_builder_end(&results);
  }

  static GVariant* start_result(bool keyboard_granted) {
    GVariantBuilder results;
    g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
    g_variant_builder_add(&results, "{sv}", "devices",
                          g_variant_new_uint32(keyboard_granted ? kKeyboardDevice : 0));
    return g_variant_builder_end(&results);
  }

  static GVariant* shortcut_result(const std::string& shortcut_id) {
    GVariantBuilder details;
    g_variant_builder_init(&details, G_VARIANT_TYPE_VARDICT);
    option_string(&details, "trigger_description", "Ctrl+Shift+V");
    GVariantBuilder shortcuts;
    g_variant_builder_init(&shortcuts, G_VARIANT_TYPE("a(sa{sv})"));
    g_variant_builder_add(&shortcuts, "(s@a{sv})", shortcut_id.c_str(),
                          g_variant_builder_end(&details));
    GVariantBuilder results;
    g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
    g_variant_builder_add(&results, "{sv}", "shortcuts",
                          g_variant_builder_end(&shortcuts));
    return g_variant_builder_end(&results);
  }

  void emit_response(GDBusConnection* server, const std::string& request_path,
                     guint response, GVariant* results = nullptr) {
    if (results == nullptr) {
      results = empty_options();
    }
    g_autoptr(GError) error = nullptr;
    const gboolean emitted = g_dbus_connection_emit_signal(
        server, nullptr, request_path.c_str(), kRequest, "Response",
        g_variant_new("(u@a{sv})", response, results),
        &error);
    g_assert_no_error(error);
    g_assert_true(emitted);
  }

  void record_call(const char* interface_name = nullptr,
                   const char* method_name = nullptr) {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      ++calls_;
      if (interface_name != nullptr && method_name != nullptr) {
        method_calls_.emplace_back(std::string(interface_name) + "." + method_name);
      }
    }
    calls_ready_.notify_all();
  }

  void record_key(gint keycode, guint state) {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      key_events_.emplace_back(keycode, state);
    }
    keys_ready_.notify_all();
  }

  static void on_method_call(GDBusConnection* server, const gchar*, const gchar*,
                             const gchar* interface_name,
                             const gchar* method_name, GVariant* parameters,
                             GDBusMethodInvocation* invocation,
                             gpointer user_data) {
    auto* fixture = static_cast<PortalFixture*>(user_data);
    if (g_strcmp0(interface_name, kProperties) == 0) {
      g_assert_cmpstr(method_name, ==, "Get");
      g_dbus_method_invocation_return_value(
          invocation, g_variant_new("(v)", g_variant_new_uint32(1)));
      return;
    }
    if (g_strcmp0(interface_name, kRemoteDesktop) == 0 &&
        g_strcmp0(method_name, "NotifyKeyboardKeycode") == 0) {
      const gchar* session = nullptr;
      GVariant* options = nullptr;
      gint keycode = 0;
      guint state = 0;
      g_variant_get(parameters, "(&o@a{sv}iu)", &session, &options, &keycode, &state);
      g_variant_unref(options);
      (void)session;
      fixture->record_key(keycode, state);
      ReplyMode mode;
      {
        std::lock_guard<std::mutex> lock(fixture->mutex_);
        mode = fixture->mode_;
      }
      if (mode == ReplyMode::kKeyDownTimeout && keycode == kEvdevLeftControl &&
          state == 1) return;
      g_dbus_method_invocation_return_value(invocation, g_variant_new("()"));
      return;
    }

    if (g_strcmp0(interface_name, kRemoteDesktop) == 0 &&
        g_strcmp0(method_name, "CreateSession") == 0) {
      const std::string path = request_path(invocation, parameters);
      g_assert_false(path.empty());
      fixture->emit_response(
          server, path, 0,
          session_result("/org/freedesktop/portal/desktop/session/remote"));
      g_dbus_method_invocation_return_value(invocation,
                                            g_variant_new("(o)", path.c_str()));
      fixture->record_call(interface_name, method_name);
      return;
    }

    if (g_strcmp0(interface_name, kRemoteDesktop) == 0 &&
        g_strcmp0(method_name, "SelectDevices") == 0) {
      const std::string path = request_path(invocation, parameters);
      g_assert_false(path.empty());
      fixture->emit_response(server, path, 0);
      g_dbus_method_invocation_return_value(invocation,
                                            g_variant_new("(o)", path.c_str()));
      fixture->record_call(interface_name, method_name);
      return;
    }

    if (g_strcmp0(interface_name, kRemoteDesktop) == 0 &&
        g_strcmp0(method_name, "Start") == 0) {
      const std::string path = request_path(invocation, parameters);
      g_assert_false(path.empty());
      bool keyboard_granted = false;
      {
        std::lock_guard<std::mutex> lock(fixture->mutex_);
        keyboard_granted = fixture->keyboard_granted_;
      }
      fixture->emit_response(server, path, 0, start_result(keyboard_granted));
      g_dbus_method_invocation_return_value(invocation,
                                            g_variant_new("(o)", path.c_str()));
      fixture->record_call(interface_name, method_name);
      return;
    }

    if (g_strcmp0(interface_name, kGlobalShortcuts) == 0 &&
        g_strcmp0(method_name, "CreateSession") == 0) {
      const std::string path = request_path(invocation, parameters);
      g_assert_false(path.empty());
      fixture->emit_response(
          server, path, 0,
          session_result("/org/freedesktop/portal/desktop/session/shortcuts"));
      g_dbus_method_invocation_return_value(invocation,
                                            g_variant_new("(o)", path.c_str()));
      fixture->record_call(interface_name, method_name);
      return;
    }

    if (g_strcmp0(interface_name, kGlobalShortcuts) == 0 &&
        g_strcmp0(method_name, "BindShortcuts") == 0) {
      const std::string path = request_path(invocation, parameters);
      g_assert_false(path.empty());
      const gchar* session = nullptr;
      const gchar* parent = nullptr;
      GVariant* shortcuts = nullptr;
      GVariant* options = nullptr;
      g_variant_get(parameters, "(&o@a(sa{sv})&s@a{sv})", &session, &shortcuts,
                    &parent, &options);
      GVariantIter iterator;
      g_variant_iter_init(&iterator, shortcuts);
      const gchar* shortcut_id = nullptr;
      GVariant* details = nullptr;
      g_assert_true(g_variant_iter_next(&iterator, "(&s@a{sv})", &shortcut_id,
                                        &details));
      const std::string requested_shortcut_id = shortcut_id;
      g_variant_unref(details);
      g_variant_unref(shortcuts);
      g_variant_unref(options);
      g_assert_cmpstr(session, ==,
                      "/org/freedesktop/portal/desktop/session/shortcuts");
      g_assert_cmpstr(parent, ==, "");
      fixture->emit_response(server, path, 0,
                             shortcut_result(requested_shortcut_id));
      g_dbus_method_invocation_return_value(invocation,
                                            g_variant_new("(o)", path.c_str()));
      fixture->record_call(interface_name, method_name);
      return;
    }

    g_assert_cmpstr(interface_name, ==, kRemoteDesktop);
    g_assert_cmpstr(method_name, ==, kTestMethod);

    const std::string path = request_path(invocation, parameters);
    g_assert_false(path.empty());

    ReplyMode mode;
    {
      std::lock_guard<std::mutex> lock(fixture->mutex_);
      mode = fixture->mode_;
    }

    if (mode == ReplyMode::kError) {
      g_dbus_method_invocation_return_dbus_error(
          invocation, "org.freedesktop.portal.Error.Failed", "fixture failure");
      fixture->record_call();
      return;
    }
    if (mode == ReplyMode::kPending) {
      fixture->record_call();
      return;
    }
    if (mode == ReplyMode::kQueuedResponse) {
      fixture->emit_response(server, path, 0);
      fixture->record_call();
      return;
    }

    // The request signal arrives before the method reply. This is permitted by
    // the portal protocol and proves the predicted-path subscriber is live.
    fixture->emit_response(server, path, 0);
    fixture->emit_response(server, path, 0);
    g_dbus_method_invocation_return_value(
        invocation, g_variant_new("(o)", path.c_str()));
    fixture->record_call();
  }

  GTestDBus* bus_ = nullptr;
  std::string bus_address_;
  GMainContext* server_context_ = nullptr;
  GMainLoop* server_loop_ = nullptr;
  GDBusConnection* server_connection_ = nullptr;
  std::thread server_thread_;
  mutable std::mutex mutex_;
  std::condition_variable ready_;
  std::condition_variable calls_ready_;
  std::condition_variable keys_ready_;
  bool server_ready_ = false;
  bool server_failed_ = false;
  std::string server_error_;
  ReplyMode mode_ = ReplyMode::kEarlyResponse;
  bool keyboard_granted_ = true;
  guint calls_ = 0;
  std::vector<std::string> method_calls_;
  std::vector<std::pair<gint, guint>> key_events_;
};

bool wait_until(const std::function<bool()>& ready) {
  const gint64 deadline = g_get_monotonic_time() + G_TIME_SPAN_SECOND;
  while (!ready() && g_get_monotonic_time() < deadline) {
    g_main_context_iteration(nullptr, false);
    g_usleep(1000);
  }
  return ready();
}

GVariant* request_options() {
  GVariantBuilder options;
  g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
  option_string(&options, "handle_token", "test_handle");
  return g_variant_new("(@a{sv})", g_variant_builder_end(&options));
}

void synchronize_subscriptions(const std::shared_ptr<LinuxPortal::Impl>& client) {
  // A round trip on the subscribing connection orders AddMatch before the
  // independent fake service emits its signal.
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) reply = g_dbus_connection_call_sync(
      client->connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
      "org.freedesktop.DBus", "GetId", nullptr, G_VARIANT_TYPE("(s)"),
      G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
  g_assert_no_error(error);
  g_assert_nonnull(reply);
}

void test_early_response_completes_once() {
  PortalFixture fixture;
  const auto client = fixture.new_client();
  guint completions = 0;
  guint response = 99;
  client->request(kRemoteDesktop, kTestMethod, request_options(),
                  [&completions, &response](guint received, GVariant*) {
                    ++completions;
                    response = received;
                  });

  g_assert_true(wait_until([&] { return completions == 1; }));
  g_assert_cmpuint(fixture.calls(), ==, 1);
  g_assert_cmpuint(completions, ==, 1);
  g_assert_cmpuint(response, ==, 0);
}

void test_error_completes_once() {
  PortalFixture fixture;
  fixture.set_mode(ReplyMode::kError);
  const auto client = fixture.new_client();
  guint completions = 0;
  guint response = 99;
  client->request(kRemoteDesktop, kTestMethod, request_options(),
                  [&completions, &response](guint received, GVariant*) {
                    ++completions;
                    response = received;
                  });

  g_assert_true(wait_until([&] { return completions == 1; }));
  g_assert_cmpuint(fixture.calls(), ==, 1);
  g_assert_cmpuint(completions, ==, 1);
  g_assert_cmpuint(response, ==, 2);
}

void test_shutdown_settles_queued_response_once() {
  PortalFixture fixture;
  fixture.set_mode(ReplyMode::kQueuedResponse);
  const auto client = fixture.new_client();
  guint completions = 0;
  guint response = 99;
  client->request(kRemoteDesktop, kTestMethod, request_options(),
                  [&completions, &response](guint received, GVariant*) {
                    ++completions;
                    response = received;
                  });

  g_assert_true(fixture.wait_for_calls(1));
  client->shutdown();
  g_assert_true(wait_until([&] { return completions == 1; }));
  g_assert_cmpuint(completions, ==, 1);
  g_assert_cmpuint(response, ==, 2);
}

void test_key_down_timeout_queues_release() {
  PortalFixture fixture;
  fixture.set_mode(ReplyMode::kKeyDownTimeout);
  LinuxPortal client(nullptr);
  client.impl_->remote_active = true;
  client.impl_->remote_session = "/org/freedesktop/portal/desktop/session/test";

  g_assert_false(client.send_ctrl_v());
  g_assert_true(fixture.wait_for_key(kEvdevLeftControl, 0));
}

void test_session_closed_unsubscribes_and_completes() {
  PortalFixture fixture;
  guint closed = 0;
  auto client = fixture.new_client();
  std::weak_ptr<LinuxPortal::Impl> weak_client = client;
  client->callbacks.session_closed = [&closed] { ++closed; };
  client->remote_active = true;
  client->observe_session("/org/freedesktop/portal/desktop/session/normal", true);
  synchronize_subscriptions(client);

  g_assert_true(fixture.emit_session_closed(
      "/org/freedesktop/portal/desktop/session/normal"));
  g_assert_true(wait_until([&] { return closed == 1; }));
  g_assert_false(client->remote_active);
  g_assert_cmpuint(client->remote_session_subscription, ==, 0);
  client.reset();
  g_assert_true(wait_until([&] { return weak_client.expired(); }));
}

void test_shutdown_drops_queued_session_closed() {
  PortalFixture fixture;
  guint closed = 0;
  auto client = fixture.new_client();
  std::weak_ptr<LinuxPortal::Impl> weak_client = client;
  client->callbacks.session_closed = [&closed] { ++closed; };
  client->observe_session("/org/freedesktop/portal/desktop/session/queued", true);
  synchronize_subscriptions(client);

  g_assert_true(fixture.emit_session_closed(
      "/org/freedesktop/portal/desktop/session/queued"));
  client->shutdown();
  client.reset();
  g_assert_true(wait_until([&] { return weak_client.expired(); }));
  g_assert_cmpuint(closed, ==, 0);
}

void test_remote_desktop_object_path_session_grants_keyboard() {
  PortalFixture fixture;
  LinuxPortal client(nullptr);
  guint completions = 0;
  bool granted = false;

  client.request_remote_desktop([&completions, &granted](bool received) {
    ++completions;
    granted = received;
  });

  g_assert_true(wait_until([&] { return completions == 1; }));
  g_assert_true(granted);
  g_assert_true(client.remote_desktop_active());
  g_assert_true(fixture.saw_sequence({
      "org.freedesktop.portal.RemoteDesktop.CreateSession",
      "org.freedesktop.portal.RemoteDesktop.SelectDevices",
      "org.freedesktop.portal.RemoteDesktop.Start",
  }));
  g_assert_cmpuint(completions, ==, 1);
}

void test_remote_desktop_denied_keyboard_never_activates() {
  PortalFixture fixture;
  fixture.set_keyboard_granted(false);
  LinuxPortal client(nullptr);
  guint completions = 0;
  bool granted = true;

  client.request_remote_desktop([&completions, &granted](bool received) {
    ++completions;
    granted = received;
  });

  g_assert_true(wait_until([&] { return completions == 1; }));
  g_assert_false(granted);
  g_assert_false(client.remote_desktop_active());
  g_assert_cmpuint(completions, ==, 1);
}

void test_global_shortcuts_object_path_session_binds_and_activates() {
  PortalFixture fixture;
  std::string activated_id;
  std::string activation_token;
  LinuxPortal client(nullptr, LinuxPortalCallbacks{
                                [&activated_id, &activation_token](
                                    const std::string& shortcut_id,
                                    const std::string& token) {
                                  activated_id = shortcut_id;
                                  activation_token = token;
                                },
                                {},
                            });
  ShortcutRequest request;
  request.id = "quick_paste";
  request.description = "Quick Paste";
  request.usage = "V";
  request.modifiers = {"Control", "Shift"};
  guint completions = 0;
  ShortcutResult result;

  client.register_shortcut(request, [&completions, &result](ShortcutResult received) {
    ++completions;
    result = std::move(received);
  });

  g_assert_true(wait_until([&] { return completions == 1; }));
  g_assert_true(result.registered);
  g_assert_cmpstr(result.trigger_description.c_str(), ==, "Ctrl+Shift+V");
  g_assert_true(client.shortcut_registered());
  g_assert_true(fixture.saw_sequence({
      "org.freedesktop.portal.GlobalShortcuts.CreateSession",
      "org.freedesktop.portal.GlobalShortcuts.BindShortcuts",
  }));
  synchronize_subscriptions(client.impl_);
  g_assert_true(fixture.emit_shortcut_activated(
      "/org/freedesktop/portal/desktop/session/shortcuts", "quick_paste",
      "fixture-activation"));
  g_assert_true(wait_until([&] { return !activated_id.empty(); }));
  g_assert_cmpstr(activated_id.c_str(), ==, "quick_paste");
  g_assert_cmpstr(activation_token.c_str(), ==, "fixture-activation");
  g_assert_cmpuint(completions, ==, 1);
}

}  // namespace

int main(int argc, char** argv) {
  g_autoptr(GError) config_error = nullptr;
  g_autofree gchar* config_home =
      g_dir_make_tmp("copypaste-linux-portal-config.XXXXXX", &config_error);
  g_assert_no_error(config_error);
  g_assert_nonnull(config_home);
  g_assert_true(g_setenv("XDG_CONFIG_HOME", config_home, true));
  g_test_init(&argc, &argv, nullptr);
  g_test_add_func("/linux_portal/early_response_completes_once",
                  test_early_response_completes_once);
  g_test_add_func("/linux_portal/error_completes_once", test_error_completes_once);
  g_test_add_func("/linux_portal/shutdown_settles_queued_response_once",
                  test_shutdown_settles_queued_response_once);
  g_test_add_func("/linux_portal/key_down_timeout_queues_release",
                  test_key_down_timeout_queues_release);
  g_test_add_func("/linux_portal/session_closed_unsubscribes_and_completes",
                  test_session_closed_unsubscribes_and_completes);
  g_test_add_func("/linux_portal/shutdown_drops_queued_session_closed",
                  test_shutdown_drops_queued_session_closed);
  g_test_add_func("/linux_portal/remote_desktop_object_path_session_grants_keyboard",
                  test_remote_desktop_object_path_session_grants_keyboard);
  g_test_add_func("/linux_portal/remote_desktop_denied_keyboard_never_activates",
                  test_remote_desktop_denied_keyboard_never_activates);
  g_test_add_func("/linux_portal/global_shortcuts_object_path_session_binds_and_activates",
                  test_global_shortcuts_object_path_session_binds_and_activates);
  const int result = g_test_run();
  g_autofree gchar* token_path =
      g_build_filename(config_home, "copypaste", "remote-desktop-token", nullptr);
  g_autofree gchar* token_directory = g_path_get_dirname(token_path);
  g_remove(token_path);
  g_rmdir(token_directory);
  g_rmdir(config_home);
  return result;
}
