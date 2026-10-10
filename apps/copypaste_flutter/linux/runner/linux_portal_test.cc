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
    const guint properties_registration = g_dbus_connection_register_object(
        server, kPortalPath, node->interfaces[1], &kVTable, this, nullptr, &error);
    if (properties_registration == 0) {
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

  void emit_response(GDBusConnection* server, const std::string& request_path,
                     guint response) {
    GVariantBuilder results;
    g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
    g_autoptr(GError) error = nullptr;
    const gboolean emitted = g_dbus_connection_emit_signal(
        server, nullptr, request_path.c_str(), kRequest, "Response",
        g_variant_new("(u@a{sv})", response, g_variant_builder_end(&results)),
        &error);
    g_assert_no_error(error);
    g_assert_true(emitted);
  }

  void record_call() {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      ++calls_;
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
    g_assert_cmpstr(interface_name, ==, kRemoteDesktop);
    if (g_strcmp0(method_name, "NotifyKeyboardKeycode") == 0) {
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
    g_assert_cmpstr(method_name, ==, kTestMethod);

    const gchar* sender = g_dbus_method_invocation_get_sender(invocation);
    g_assert_nonnull(sender);
    std::string sender_path(sender + 1);
    for (char& character : sender_path) {
      if (!g_ascii_isalnum(character)) character = '_';
    }
    const std::string request_path = std::string(kPortalPath) + "/request/" +
                                     sender_path + "/test_handle";

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
      fixture->emit_response(server, request_path, 0);
      fixture->record_call();
      return;
    }

    // The request signal arrives before the method reply. This is permitted by
    // the portal protocol and proves the predicted-path subscriber is live.
    fixture->emit_response(server, request_path, 0);
    fixture->emit_response(server, request_path, 0);
    g_dbus_method_invocation_return_value(
        invocation, g_variant_new("(o)", request_path.c_str()));
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
  guint calls_ = 0;
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

}  // namespace

int main(int argc, char** argv) {
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
  return g_test_run();
}
