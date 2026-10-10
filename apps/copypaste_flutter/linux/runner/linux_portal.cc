#include "linux_portal.h"

#include <gdk/gdk.h>
#if defined(GDK_WINDOWING_WAYLAND)
#include <gdk/gdkwayland.h>
#endif
#if defined(GDK_WINDOWING_X11)
#include <gdk/gdkx.h>
#endif
#include <gio/gio.h>
#include <glib/gstdio.h>
#include <gtk/gtk.h>

#include <algorithm>
#include <atomic>
#include <cctype>
#include <cstdint>
#include <memory>
#include <string>
#include <utility>
#include <vector>

namespace {

constexpr char kPortalName[] = "org.freedesktop.portal.Desktop";
constexpr char kPortalPath[] = "/org/freedesktop/portal/desktop";
constexpr char kProperties[] = "org.freedesktop.DBus.Properties";
constexpr char kRequest[] = "org.freedesktop.portal.Request";
constexpr char kSession[] = "org.freedesktop.portal.Session";
constexpr char kGlobalShortcuts[] = "org.freedesktop.portal.GlobalShortcuts";
constexpr char kRemoteDesktop[] = "org.freedesktop.portal.RemoteDesktop";
constexpr guint kKeyboardDevice = 1;
constexpr guint kPersistUntilRevoked = 2;
constexpr gint kEvdevLeftControl = 29;
constexpr gint kEvdevV = 47;

std::atomic_uint64_t next_token{0};

std::string token(const char* purpose) {
  return std::string("copypaste_") + purpose + "_" +
         std::to_string(++next_token) + "_" +
         std::to_string(g_random_int());
}

GVariant* empty_options() {
  GVariantBuilder builder;
  g_variant_builder_init(&builder, G_VARIANT_TYPE_VARDICT);
  return g_variant_builder_end(&builder);
}

void option_string(GVariantBuilder* builder, const char* key,
                   const std::string& value) {
  g_variant_builder_add(builder, "{sv}", key, g_variant_new_string(value.c_str()));
}

void option_uint(GVariantBuilder* builder, const char* key, guint value) {
  g_variant_builder_add(builder, "{sv}", key, g_variant_new_uint32(value));
}

std::string variant_string(GVariant* dictionary, const char* key) {
  if (dictionary == nullptr) {
    return {};
  }
  g_autoptr(GVariant) value =
      g_variant_lookup_value(dictionary, key, G_VARIANT_TYPE_STRING);
  return value == nullptr ? std::string() : g_variant_get_string(value, nullptr);
}

std::string variant_object_path(GVariant* dictionary, const char* key) {
  if (dictionary == nullptr) {
    return {};
  }
  g_autoptr(GVariant) value = g_variant_lookup_value(dictionary, key, nullptr);
  if (value == nullptr ||
      (!g_variant_is_of_type(value, G_VARIANT_TYPE_OBJECT_PATH) &&
       !g_variant_is_of_type(value, G_VARIANT_TYPE_STRING))) {
    return {};
  }
  const gchar* path = g_variant_get_string(value, nullptr);
  return path != nullptr && g_variant_is_object_path(path) ? path : std::string();
}

guint variant_uint(GVariant* dictionary, const char* key) {
  if (dictionary == nullptr) {
    return 0;
  }
  g_autoptr(GVariant) value =
      g_variant_lookup_value(dictionary, key, G_VARIANT_TYPE_UINT32);
  return value == nullptr ? 0 : g_variant_get_uint32(value);
}

bool portal_interface_available(GDBusConnection* connection,
                                const char* interface_name) {
  if (connection == nullptr) {
    return false;
  }
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) result = g_dbus_connection_call_sync(
      connection, kPortalName, kPortalPath, kProperties, "Get",
      g_variant_new("(ss)", interface_name, "version"),
      G_VARIANT_TYPE("(v)"), G_DBUS_CALL_FLAGS_NONE, -1, nullptr, &error);
  return result != nullptr;
}

std::string restore_token_path() {
  return std::string(g_get_user_config_dir()) + "/copypaste/remote-desktop-token";
}

std::string read_restore_token() {
  const std::string path = restore_token_path();
  gchar* contents = nullptr;
  gsize length = 0;
  if (!g_file_get_contents(path.c_str(), &contents, &length, nullptr)) {
    return {};
  }
  std::unique_ptr<gchar, decltype(&g_free)> holder(contents, g_free);
  if (length == 0 || length > 4096 ||
      std::any_of(contents, contents + length,
                  [](gchar value) { return value == '\0' || value == '\n'; })) {
    return {};
  }
  return std::string(contents, length);
}

void save_restore_token(const std::string& value) {
  if (value.empty() || value.size() > 4096 || value.find('\n') != std::string::npos ||
      value.find('\0') != std::string::npos) {
    return;
  }
  g_autofree gchar* directory = g_path_get_dirname(restore_token_path().c_str());
  if (g_mkdir_with_parents(directory, 0700) != 0) {
    return;
  }
  g_autoptr(GError) error = nullptr;
  if (g_file_set_contents(restore_token_path().c_str(), value.c_str(), value.size(),
                          &error)) {
    g_chmod(restore_token_path().c_str(), 0600);
  }
}

std::string preferred_trigger(const ShortcutRequest& request) {
  if (!request.preferred_trigger.empty()) {
    return request.preferred_trigger;
  }

  std::string modifiers;
  for (const std::string& modifier : request.modifiers) {
    std::string normalized;
    normalized.reserve(modifier.size());
    for (const char character : modifier) {
      normalized += static_cast<char>(g_ascii_tolower(character));
    }
    if (normalized == "control" || normalized == "ctrl") {
      modifiers += "<Control>";
    } else if (normalized == "shift") {
      modifiers += "<Shift>";
    } else if (normalized == "alt") {
      modifiers += "<Alt>";
    } else if (normalized == "super" || normalized == "meta") {
      modifiers += "<Super>";
    }
  }

  // USB HID keyboard page usages for the keys offered by the current Flutter
  // shortcut schema. Unknown usages deliberately omit the hint: portals must
  // not be given an invented binding.
  std::string key;
  if (request.usage.size() == 1 &&
      std::isalpha(static_cast<unsigned char>(request.usage.front()))) {
    key = static_cast<char>(g_ascii_toupper(request.usage.front()));
  } else if (request.usage.size() == 4 &&
             request.usage.rfind("Key", 0) == 0 &&
             std::isalpha(static_cast<unsigned char>(request.usage[3]))) {
    key = static_cast<char>(g_ascii_toupper(request.usage[3]));
  } else if (request.usage == "USB_HID_0x19" || request.usage == "KeyboardV") {
    key = "V";
  }
  return key.empty() ? std::string() : modifiers + key;
}

std::string request_path_for(GDBusConnection* connection, GVariant* parameters) {
  if (connection == nullptr || parameters == nullptr) return {};
  std::string handle_token;
  const gsize children = g_variant_n_children(parameters);
  for (gsize index = 0; index < children; ++index) {
    g_autoptr(GVariant) child = g_variant_get_child_value(parameters, index);
    if (g_variant_is_of_type(child, G_VARIANT_TYPE_VARDICT)) {
      handle_token = variant_string(child, "handle_token");
      if (!handle_token.empty()) break;
    }
  }
  const gchar* sender = g_dbus_connection_get_unique_name(connection);
  if (sender == nullptr || *sender != ':' || handle_token.empty()) return {};
  for (const char value : handle_token) {
    if (!g_ascii_isalnum(value) && value != '_') return {};
  }
  std::string sender_path(sender + 1);
  for (char& value : sender_path) {
    if (!g_ascii_isalnum(value)) value = '_';
  }
  return std::string(kPortalPath) + "/request/" + sender_path + "/" + handle_token;
}

}  // namespace

struct LinuxPortal::Impl : public std::enable_shared_from_this<LinuxPortal::Impl> {
  struct ResponseGate {
    std::atomic_bool settled{false};
    std::function<void(guint, GVariant*)> complete;
  };

  struct PendingResponse {
    std::shared_ptr<Impl> owner;
    guint subscription = 0;
    std::shared_ptr<ResponseGate> gate;
  };

  struct AsyncCall {
    std::shared_ptr<Impl> owner;
    std::function<void(GVariant*, GError*)> complete;
  };

  explicit Impl(GtkWindow* initial_parent, LinuxPortalCallbacks initial_callbacks)
      : parent(initial_parent == nullptr
                   ? nullptr
                   : GTK_WINDOW(g_object_ref(initial_parent))),
        callbacks(std::move(initial_callbacks)),
        cancellable(g_cancellable_new()) {
    g_autoptr(GError) error = nullptr;
    connection = g_bus_get_sync(G_BUS_TYPE_SESSION, cancellable, &error);
    global_shortcuts_available =
        portal_interface_available(connection, kGlobalShortcuts);
    remote_desktop_available = portal_interface_available(connection, kRemoteDesktop);
  }

  ~Impl() {
    shutdown();
    g_clear_object(&connection);
    g_clear_object(&cancellable);
    g_clear_object(&parent);
  }

  void shutdown() {
    if (cancelled) {
      return;
    }
    cancelled = true;
    g_cancellable_cancel(cancellable);
    // Copy the gates first: completion is allowed to re-enter the host and
    // therefore must not iterate a vector it can mutate.
    const auto gates = pending_gates;
    for (const auto& gate : gates) {
      complete_once(gate, 2, nullptr);
    }
    if (connection != nullptr && shortcut_activation_subscription != 0) {
      g_dbus_connection_signal_unsubscribe(connection,
                                           shortcut_activation_subscription);
      shortcut_activation_subscription = 0;
    }
    close_session(shortcut_session, shortcut_session_subscription);
    close_session(remote_session, remote_session_subscription);
  }

  void close_session(std::string& session, guint& subscription) {
    if (connection != nullptr && subscription != 0) {
      g_dbus_connection_signal_unsubscribe(connection, subscription);
      subscription = 0;
    }
    if (connection != nullptr && !session.empty()) {
      g_dbus_connection_call(connection, kPortalName, session.c_str(), kSession,
                             "Close", nullptr, nullptr,
                             G_DBUS_CALL_FLAGS_NONE, -1, nullptr, nullptr,
                             nullptr);
    }
    session.clear();
  }

  void call(const char* interface_name, const char* method, GVariant* parameters,
            std::function<void(GVariant*, GError*)> complete) {
    if (cancelled || connection == nullptr) {
      complete(nullptr, nullptr);
      return;
    }
    auto* call = new AsyncCall{shared_from_this(), std::move(complete)};
    g_dbus_connection_call(connection, kPortalName, kPortalPath, interface_name,
                           method, parameters, nullptr,
                           G_DBUS_CALL_FLAGS_NONE, -1, cancellable,
                           [](GObject* source, GAsyncResult* result,
                              gpointer data) {
                             std::unique_ptr<AsyncCall> pending(
                                 static_cast<AsyncCall*>(data));
                             g_autoptr(GError) error = nullptr;
                             g_autoptr(GVariant) response =
                                 g_dbus_connection_call_finish(
                                     G_DBUS_CONNECTION(source), result, &error);
                             // A request owns a Flutter continuation. Its gate
                             // settles it during cancellation; invoke this
                             // continuation as well so an unobserved request
                             // cannot retain a captured callback.
                             pending->complete(response, error);
                           },
                           call);
  }

  void request(const char* interface_name, const char* method, GVariant* parameters,
               std::function<void(guint, GVariant*)> complete) {
    auto gate = std::make_shared<ResponseGate>();
    gate->complete = std::move(complete);
    pending_gates.push_back(gate);
    const std::string predicted = request_path_for(connection, parameters);
    if (!predicted.empty()) watch_response(predicted.c_str(), gate);
    call(interface_name, method, parameters,
         [owner = shared_from_this(), predicted, gate](
             GVariant* response, GError* error) {
           if (gate->settled.load()) return;
           if (error != nullptr || response == nullptr) {
             owner->complete_once(gate, 2, nullptr);
             return;
           }
           if (!g_variant_is_of_type(response, G_VARIANT_TYPE("(o)"))) {
             owner->complete_once(gate, 2, nullptr);
             return;
           }
           const gchar* request_path = nullptr;
           g_variant_get(response, "(&o)", &request_path);
           if (predicted.empty() || predicted != request_path) {
             // The portal spec defines the predicted path from handle_token.
             // Retain one fallback for an older portal, while the shared gate
             // guarantees that an immediate Response can complete only once.
             owner->watch_response(request_path, gate);
           }
         });
  }

  void remove_pending(PendingResponse* target) {
    const auto iterator = std::find(pending_responses.begin(),
                                    pending_responses.end(), target);
    if (iterator == pending_responses.end()) return;
    pending_responses.erase(iterator);
    if (connection != nullptr && target->subscription != 0) {
      g_dbus_connection_signal_unsubscribe(connection, target->subscription);
    }
    // GDBus owns target until its destroy notify runs. A duplicate signal may
    // already be queued when this subscription is removed.
  }

  void complete_once(const std::shared_ptr<ResponseGate>& gate,
                     guint response, GVariant* results) {
    if (gate == nullptr || gate->settled.exchange(true)) return;

    // Predicted and fallback request paths may both be subscribed. Remove all
    // of them before invoking user code, which can synchronously destroy the
    // portal or issue another request.
    std::vector<PendingResponse*> matches;
    for (PendingResponse* pending : pending_responses) {
      if (pending->gate == gate) matches.push_back(pending);
    }
    for (PendingResponse* pending : matches) remove_pending(pending);

    pending_gates.erase(
        std::remove(pending_gates.begin(), pending_gates.end(), gate),
        pending_gates.end());
    gate->complete(response, results);
  }

  void watch_response(const char* request_path,
                      const std::shared_ptr<ResponseGate>& gate) {
    if (request_path == nullptr || *request_path == '\0' || connection == nullptr ||
        cancelled) {
      complete_once(gate, 2, nullptr);
      return;
    }
    auto* pending = new PendingResponse{shared_from_this(), 0, gate};
    pending->subscription = g_dbus_connection_signal_subscribe(
        connection, kPortalName, kRequest, "Response", request_path, nullptr,
        G_DBUS_SIGNAL_FLAGS_NONE,
        [](GDBusConnection*, const gchar*, const gchar*, const gchar*,
           const gchar*, GVariant* parameters, gpointer data) {
          auto* pending = static_cast<PendingResponse*>(data);
          const std::shared_ptr<Impl> owner = pending->owner;
          guint response = 2;
          GVariant* results = nullptr;
          g_variant_get(parameters, "(u@a{sv})", &response, &results);
          const auto gate = pending->gate;
          owner->remove_pending(pending);
          // Every request owns an async Flutter/D-Bus continuation. Complete
          // it even during teardown so callers never retain a hanging call.
          owner->complete_once(gate, owner->cancelled ? 2 : response,
                               owner->cancelled ? nullptr : results);
          if (results != nullptr) {
            g_variant_unref(results);
          }
        },
        pending, [](gpointer data) {
          delete static_cast<PendingResponse*>(data);
        });
    if (pending->subscription == 0) {
      delete pending;
      complete_once(gate, 2, nullptr);
      return;
    }
    pending_responses.push_back(pending);
  }

  void with_parent_identifier(std::function<void(std::string)> complete) {
    if (parent == nullptr) {
      complete({});
      return;
    }
    GdkWindow* window = gtk_widget_get_window(GTK_WIDGET(parent));
    if (window == nullptr) {
      complete({});
      return;
    }
#if defined(GDK_WINDOWING_X11)
    if (GDK_IS_X11_DISPLAY(gdk_window_get_display(window))) {
      complete("x11:" + std::to_string(GDK_WINDOW_XID(window)));
      return;
    }
#endif
#if defined(GDK_WINDOWING_WAYLAND)
    if (GDK_IS_WAYLAND_DISPLAY(gdk_window_get_display(window))) {
      struct Export {
        std::shared_ptr<Impl> owner;
        std::function<void(std::string)> complete;
      };
      // Keep the local completion callable for synchronous export failure;
      // Export receives its own copy and owns the eventual callback path.
      auto* export_request = new Export{shared_from_this(), complete};
      if (!gdk_wayland_window_export_handle(
              window,
              [](GdkWindow* exported_window, const char* handle, gpointer data) {
                std::unique_ptr<Export> request(static_cast<Export*>(data));
                if (request->owner->cancelled || handle == nullptr) {
                  request->complete({});
                  return;
                }
                request->complete("wayland:" + std::string(handle));
                gdk_wayland_window_unexport_handle(exported_window);
              },
              export_request, nullptr)) {
        delete export_request;
        complete({});
      }
      return;
    }
#endif
    complete({});
  }

  void observe_session(const std::string& path, bool remote) {
    guint* subscription = remote ? &remote_session_subscription
                                 : &shortcut_session_subscription;
    std::string* session = remote ? &remote_session : &shortcut_session;
    *session = path;
    auto* callback_owner = new std::shared_ptr<Impl>(shared_from_this());
    *subscription = g_dbus_connection_signal_subscribe(
        connection, kPortalName, kSession, "Closed", path.c_str(), nullptr,
        G_DBUS_SIGNAL_FLAGS_NONE,
        [](GDBusConnection*, const gchar*, const gchar* object_path,
           const gchar*, const gchar*, GVariant*, gpointer data) {
          const auto owner = *static_cast<std::shared_ptr<Impl>*>(data);
          if (owner->remote_session == object_path) {
            owner->remote_active = false;
            owner->remote_session.clear();
            if (owner->connection != nullptr &&
                owner->remote_session_subscription != 0) {
              g_dbus_connection_signal_unsubscribe(
                  owner->connection, owner->remote_session_subscription);
            }
            owner->remote_session_subscription = 0;
            if (owner->callbacks.session_closed != nullptr) {
              owner->callbacks.session_closed();
            }
          }
          if (owner->shortcut_session == object_path) {
            owner->shortcut_bound = false;
            owner->shortcut_id.clear();
            owner->shortcut_session.clear();
            if (owner->connection != nullptr &&
                owner->shortcut_session_subscription != 0) {
              g_dbus_connection_signal_unsubscribe(
                  owner->connection, owner->shortcut_session_subscription);
            }
            owner->shortcut_session_subscription = 0;
          }
        },
        callback_owner, [](gpointer data) {
          delete static_cast<std::shared_ptr<Impl>*>(data);
        });
  }

  GtkWindow* parent = nullptr;
  LinuxPortalCallbacks callbacks;
  GDBusConnection* connection = nullptr;
  GCancellable* cancellable = nullptr;
  bool cancelled = false;
  bool global_shortcuts_available = false;
  bool remote_desktop_available = false;
  bool remote_request_in_flight = false;
  bool remote_active = false;
  bool shortcut_bound = false;
  std::string remote_session;
  std::string shortcut_session;
  std::string shortcut_id;
  guint remote_session_subscription = 0;
  guint shortcut_session_subscription = 0;
  guint shortcut_activation_subscription = 0;
  std::vector<PendingResponse*> pending_responses;
  std::vector<std::shared_ptr<ResponseGate>> pending_gates;
};

LinuxPortal::LinuxPortal(GtkWindow* parent, LinuxPortalCallbacks callbacks)
    : impl_(std::make_shared<Impl>(parent, std::move(callbacks))) {}

LinuxPortal::~LinuxPortal() {
  impl_->shutdown();
}

bool LinuxPortal::is_available() const {
  return impl_->global_shortcuts_available;
}

bool LinuxPortal::quick_paste_ready() const {
  return impl_->remote_active && impl_->shortcut_bound;
}

bool LinuxPortal::remote_desktop_active() const { return impl_->remote_active; }

bool LinuxPortal::remote_desktop_available() const {
  return impl_->remote_desktop_available;
}

bool LinuxPortal::shortcut_registered() const { return impl_->shortcut_bound; }

void LinuxPortal::request_remote_desktop(std::function<void(bool)> done) {
  const std::shared_ptr<Impl> portal = impl_;
  if (!portal->remote_desktop_available || portal->remote_request_in_flight) {
    done(false);
    return;
  }
  if (portal->remote_active) {
    done(true);
    return;
  }
  portal->remote_request_in_flight = true;
  portal->with_parent_identifier(
      [portal, done = std::move(done)](std::string parent_identifier) mutable {
        if (parent_identifier.empty() && portal->parent != nullptr) {
          portal->remote_request_in_flight = false;
          done(false);
          return;
        }
        GVariantBuilder options;
        g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
        option_string(&options, "handle_token", token("remote_create"));
        option_string(&options, "session_handle_token", token("remote_session"));
        portal->request(kRemoteDesktop, "CreateSession",
                        g_variant_new("(@a{sv})", g_variant_builder_end(&options)),
                        [portal, parent_identifier = std::move(parent_identifier),
                         done = std::move(done)](guint response,
                                                  GVariant* results) mutable {
          if (response != 0 || results == nullptr) {
            portal->remote_request_in_flight = false;
            done(false);
            return;
          }
          const std::string session =
              variant_object_path(results, "session_handle");
          if (session.empty()) {
            portal->remote_request_in_flight = false;
            done(false);
            return;
          }
          portal->observe_session(session, true);
          GVariantBuilder options;
          g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
          option_string(&options, "handle_token", token("remote_devices"));
          option_uint(&options, "types", kKeyboardDevice);
          option_uint(&options, "persist_mode", kPersistUntilRevoked);
          const std::string restore = read_restore_token();
          if (!restore.empty()) {
            option_string(&options, "restore_token", restore);
          }
          portal->request(kRemoteDesktop, "SelectDevices",
                          g_variant_new("(o@a{sv})", session.c_str(),
                                        g_variant_builder_end(&options)),
                          [portal, session, parent_identifier = std::move(parent_identifier),
                           done = std::move(done)](guint select_response,
                                                    GVariant*) mutable {
            if (select_response != 0) {
              portal->remote_request_in_flight = false;
              portal->close_session(portal->remote_session,
                                    portal->remote_session_subscription);
              done(false);
              return;
            }
            GVariantBuilder options;
            g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
            option_string(&options, "handle_token", token("remote_start"));
            portal->request(kRemoteDesktop, "Start",
                            g_variant_new("(os@a{sv})", session.c_str(),
                                          parent_identifier.c_str(),
                                          g_variant_builder_end(&options)),
                            [portal, done = std::move(done)](guint start_response,
                                                              GVariant* start_results) mutable {
              portal->remote_request_in_flight = false;
              const bool keyboard_granted =
                  start_response == 0 &&
                  (variant_uint(start_results, "devices") & kKeyboardDevice) != 0;
              portal->remote_active = keyboard_granted;
              if (keyboard_granted) {
                save_restore_token(variant_string(start_results, "restore_token"));
              } else {
                portal->close_session(portal->remote_session,
                                      portal->remote_session_subscription);
              }
              done(keyboard_granted);
            });
          });
        });
      });
}

void LinuxPortal::register_shortcut(const ShortcutRequest& request,
                                    std::function<void(ShortcutResult)> done) {
  const std::shared_ptr<Impl> portal = impl_;
  if (!portal->global_shortcuts_available || request.id.empty() ||
      request.description.empty()) {
    done({false, {}, "Global shortcuts are unavailable."});
    return;
  }

  // BindShortcuts can only be invoked once per session. Replacing a configured
  // shortcut therefore closes its old session before making a new user-visible
  // request.
  portal->close_session(portal->shortcut_session,
                        portal->shortcut_session_subscription);
  if (portal->shortcut_activation_subscription != 0) {
    g_dbus_connection_signal_unsubscribe(portal->connection,
                                         portal->shortcut_activation_subscription);
    portal->shortcut_activation_subscription = 0;
  }
  portal->shortcut_bound = false;
  portal->shortcut_id.clear();
  portal->with_parent_identifier(
      [portal, request, done = std::move(done)](std::string parent_identifier) mutable {
        if (parent_identifier.empty() && portal->parent != nullptr) {
          done({false, {}, "The application window is not ready for portal consent."});
          return;
        }
        GVariantBuilder options;
        g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
        option_string(&options, "handle_token", token("shortcut_create"));
        option_string(&options, "session_handle_token", token("shortcut_session"));
        portal->request(kGlobalShortcuts, "CreateSession",
                        g_variant_new("(@a{sv})", g_variant_builder_end(&options)),
                        [portal, request, parent_identifier = std::move(parent_identifier),
                         done = std::move(done)](guint response,
                                                  GVariant* results) mutable {
          const std::string session = response == 0
              ? variant_object_path(results, "session_handle") : std::string();
          if (session.empty()) {
            done({false, {}, "The portal did not create a shortcut session."});
            return;
          }
          portal->observe_session(session, false);
          GVariantBuilder details;
          g_variant_builder_init(&details, G_VARIANT_TYPE_VARDICT);
          option_string(&details, "description", request.description);
          const std::string trigger = preferred_trigger(request);
          if (!trigger.empty()) {
            option_string(&details, "preferred_trigger", trigger);
          }
          GVariantBuilder shortcuts;
          g_variant_builder_init(&shortcuts, G_VARIANT_TYPE("a(sa{sv})"));
          g_variant_builder_add(&shortcuts, "(s@a{sv})", request.id.c_str(),
                                g_variant_builder_end(&details));
          GVariantBuilder options;
          g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
          option_string(&options, "handle_token", token("shortcut_bind"));
          portal->request(kGlobalShortcuts, "BindShortcuts",
                          g_variant_new("(o@a(sa{sv})s@a{sv})", session.c_str(),
                                        g_variant_builder_end(&shortcuts),
                                        parent_identifier.c_str(),
                                        g_variant_builder_end(&options)),
                          [portal, session, request, done = std::move(done)](
                              guint bind_response, GVariant* bind_results) mutable {
            std::string trigger_description;
            if (bind_response == 0 && bind_results != nullptr) {
              g_autoptr(GVariant) shortcuts = g_variant_lookup_value(
                  bind_results, "shortcuts", G_VARIANT_TYPE("a(sa{sv})"));
              if (shortcuts != nullptr) {
                GVariantIter iterator;
                g_variant_iter_init(&iterator, shortcuts);
                const gchar* id = nullptr;
                GVariant* details = nullptr;
                while (g_variant_iter_next(&iterator, "(&s@a{sv})", &id, &details)) {
                  if (request.id == id) {
                    trigger_description = variant_string(details, "trigger_description");
                  }
                  g_variant_unref(details);
                }
              }
            }
            if (trigger_description.empty()) {
              portal->close_session(portal->shortcut_session,
                                    portal->shortcut_session_subscription);
              done({false, {}, "The portal did not bind the requested shortcut."});
              return;
            }
            portal->shortcut_id = request.id;
            portal->shortcut_bound = true;
            auto* callback_owner =
                new std::shared_ptr<LinuxPortal::Impl>(portal);
            portal->shortcut_activation_subscription =
                g_dbus_connection_signal_subscribe(
                    portal->connection, kPortalName, kGlobalShortcuts, "Activated",
                    kPortalPath, nullptr, G_DBUS_SIGNAL_FLAGS_NONE,
                    [](GDBusConnection*, const gchar*, const gchar*, const gchar*,
                       const gchar*, GVariant* parameters, gpointer data) {
                      const auto owner =
                          *static_cast<std::shared_ptr<LinuxPortal::Impl>*>(data);
                      const gchar* session = nullptr;
                      const gchar* id = nullptr;
                      guint64 timestamp = 0;
                      GVariant* options = nullptr;
                      g_variant_get(parameters, "(&o&st@a{sv})", &session, &id,
                                    &timestamp, &options);
                      if (!owner->cancelled && owner->shortcut_session == session &&
                          owner->callbacks.shortcut_activated != nullptr) {
                        owner->callbacks.shortcut_activated(
                            id, variant_string(options, "activation_token"));
                      }
                      g_variant_unref(options);
                    },
                    callback_owner, [](gpointer data) {
                      delete static_cast<std::shared_ptr<LinuxPortal::Impl>*>(data);
                    });
            done({true, trigger_description, {}});
          });
        });
      });
}

void LinuxPortal::unregister_shortcut(const std::string& id,
                                      std::function<void(bool)> done) {
  const std::shared_ptr<Impl> portal = impl_;
  if (portal->shortcut_id != id && !portal->shortcut_id.empty()) {
    done(false);
    return;
  }
  if (portal->shortcut_activation_subscription != 0) {
    g_dbus_connection_signal_unsubscribe(portal->connection,
                                         portal->shortcut_activation_subscription);
    portal->shortcut_activation_subscription = 0;
  }
  portal->close_session(portal->shortcut_session,
                        portal->shortcut_session_subscription);
  portal->shortcut_id.clear();
  portal->shortcut_bound = false;
  done(true);
}

bool LinuxPortal::send_ctrl_v() {
  const std::shared_ptr<Impl> portal = impl_;
  if (!portal->remote_active || portal->remote_session.empty() ||
      portal->connection == nullptr) {
    return false;
  }
  struct KeyNotification {
    bool attempted = false;
    bool delivered = false;
  };
  const gint64 deadline = g_get_monotonic_time() + 2 * G_TIME_SPAN_SECOND;
  const auto send_key = [portal, deadline](gint keycode, guint state) -> KeyNotification {
    const gint64 remaining = deadline - g_get_monotonic_time();
    if (remaining <= 0) return {};
    const gint timeout_milliseconds = static_cast<gint>(
        std::max<gint64>(1, (remaining + G_TIME_SPAN_MILLISECOND - 1) /
                               G_TIME_SPAN_MILLISECOND));
    g_autoptr(GError) error = nullptr;
    g_autoptr(GVariant) result = g_dbus_connection_call_sync(
        portal->connection, kPortalName, kPortalPath, kRemoteDesktop,
        "NotifyKeyboardKeycode",
        g_variant_new("(o@a{sv}iu)", portal->remote_session.c_str(),
                      empty_options(), keycode, state), nullptr,
        G_DBUS_CALL_FLAGS_NONE, timeout_milliseconds, portal->cancellable, &error);
    if (result == nullptr) portal->remote_active = false;
    return {true, result != nullptr};
  };
  const auto queue_key_release = [portal](gint keycode) {
    // This is deliberately asynchronous: a release still matters after the
    // foreground action's absolute deadline, but must not extend that action.
    g_dbus_connection_call(
        portal->connection, kPortalName, kPortalPath, kRemoteDesktop,
        "NotifyKeyboardKeycode",
        g_variant_new("(o@a{sv}iu)", portal->remote_session.c_str(),
                      empty_options(), keycode, 0), nullptr,
        G_DBUS_CALL_FLAGS_NONE, -1, nullptr, nullptr, nullptr);
  };

  // Do not leave a modifier pressed if a later RPC fails. Every release shares
  // the transaction deadline, so cleanup never extends the two-second user
  // action budget.
  const KeyNotification control_down = send_key(kEvdevLeftControl, 1);
  const KeyNotification v_down = control_down.delivered
      ? send_key(kEvdevV, 1) : KeyNotification{};
  // A timed-out key-down can still have reached the compositor. Release every
  // key whose down event was attempted, while there is transaction budget.
  const KeyNotification v_up = v_down.attempted
      ? send_key(kEvdevV, 0) : KeyNotification{};
  const KeyNotification control_up = control_down.attempted
      ? send_key(kEvdevLeftControl, 0) : KeyNotification{};
  if (v_down.attempted && !v_up.delivered) queue_key_release(kEvdevV);
  if (control_down.attempted && !control_up.delivered) {
    queue_key_release(kEvdevLeftControl);
  }
  return control_down.delivered && v_down.delivered && v_up.delivered &&
         control_up.delivered;
}
