#include "linux_gnome_shortcuts.h"

#include <gio/gio.h>

#include <cstdlib>
#include <memory>
#include <string>
#include <utility>

namespace {

constexpr char kCompanionName[] = "app.copypaste.GnomeIntegration";
constexpr char kCompanionPath[] = "/app/copypaste/GnomeShortcuts";
constexpr char kCompanionInterface[] = "app.copypaste.GnomeShortcuts";
constexpr gint kRpcTimeoutMilliseconds = 2000;

std::string gtk_accelerator(const ShortcutRequest& request) {
  if (request.usage.empty() || request.modifiers.empty()) return {};
  char* end = nullptr;
  const unsigned long usage = std::strtoul(request.usage.c_str(), &end, 10);
  if (end == request.usage.c_str() || *end != '\0' ||
      (usage & ~0xffffUL) != 0x70000UL) {
    return {};
  }
  const unsigned long key_usage = usage & 0xffffUL;
  std::string key;
  if (key_usage >= 0x04 && key_usage <= 0x1d) {
    key.assign(1, static_cast<char>('a' + key_usage - 0x04));
  } else if (key_usage >= 0x1e && key_usage <= 0x26) {
    key.assign(1, static_cast<char>('1' + key_usage - 0x1e));
  } else if (key_usage == 0x27) {
    key = "0";
  } else if (key_usage >= 0x3a && key_usage <= 0x45) {
    key = "F" + std::to_string(key_usage - 0x3a + 1);
  } else if (key_usage >= 0x68 && key_usage <= 0x73) {
    key = "F" + std::to_string(key_usage - 0x68 + 13);
  } else {
    switch (key_usage) {
      case 0x28: key = "Return"; break;
      case 0x29: key = "Escape"; break;
      case 0x2a: key = "BackSpace"; break;
      case 0x2b: key = "Tab"; break;
      case 0x2c: key = "space"; break;
      case 0x2d: key = "minus"; break;
      case 0x2e: key = "equal"; break;
      case 0x2f: key = "bracketleft"; break;
      case 0x30: key = "bracketright"; break;
      case 0x31: key = "backslash"; break;
      case 0x33: key = "semicolon"; break;
      case 0x34: key = "apostrophe"; break;
      case 0x35: key = "grave"; break;
      case 0x36: key = "comma"; break;
      case 0x37: key = "period"; break;
      case 0x38: key = "slash"; break;
      case 0x49: key = "Insert"; break;
      case 0x4a: key = "Home"; break;
      case 0x4b: key = "Page_Up"; break;
      case 0x4c: key = "Delete"; break;
      case 0x4d: key = "End"; break;
      case 0x4e: key = "Page_Down"; break;
      case 0x4f: key = "Right"; break;
      case 0x50: key = "Left"; break;
      case 0x51: key = "Down"; break;
      case 0x52: key = "Up"; break;
      default: return {};
    }
  }
  std::string result;
  bool control = false;
  bool shift = false;
  bool alt = false;
  bool meta = false;
  for (const std::string& modifier : request.modifiers) {
    if (modifier == "control") {
      if (control) return {};
      control = true;
      result += "<Control>";
    } else if (modifier == "shift") {
      if (shift) return {};
      shift = true;
      result += "<Shift>";
    } else if (modifier == "alt") {
      if (alt) return {};
      alt = true;
      result += "<Alt>";
    } else if (modifier == "meta") {
      if (meta) return {};
      meta = true;
      result += "<Super>";
    } else {
      return {};
    }
  }
  return result + key;
}

}  // namespace

struct LinuxGnomeShortcuts::Impl
    : public std::enable_shared_from_this<LinuxGnomeShortcuts::Impl> {
  struct VersionCall {
    std::shared_ptr<Impl> owner;
    guint generation = 0;
  };

  struct RegisterCall {
    std::shared_ptr<Impl> owner;
    guint generation = 0;
    std::string id;
    std::function<void(ShortcutResult)> done;
  };

  struct UnregisterCall {
    std::shared_ptr<Impl> owner;
    guint generation = 0;
    std::function<void(bool)> done;
  };

  struct ActivationSubscription {
    std::shared_ptr<Impl> owner;
    guint registration_epoch = 0;
  };

  explicit Impl(std::function<void(const std::string&)> initial_activated)
      : activated(std::move(initial_activated)), cancellable(g_cancellable_new()) {
    g_autoptr(GError) error = nullptr;
    connection = g_bus_get_sync(G_BUS_TYPE_SESSION, cancellable, &error);
  }

  ~Impl() { shutdown(); }

  void start() {
    if (connection == nullptr || !active) return;
    auto* owner = new std::shared_ptr<Impl>(shared_from_this());
    name_owner_subscription = g_dbus_connection_signal_subscribe(
        connection, "org.freedesktop.DBus", "org.freedesktop.DBus",
        "NameOwnerChanged", "/org/freedesktop/DBus", kCompanionName,
        G_DBUS_SIGNAL_FLAGS_NONE,
        [](GDBusConnection*, const gchar*, const gchar*, const gchar*,
           const gchar*, GVariant* parameters, gpointer data) {
          const auto self = *static_cast<std::shared_ptr<Impl>*>(data);
          const gchar* name = nullptr;
          const gchar* old_owner = nullptr;
          const gchar* new_owner = nullptr;
          g_variant_get(parameters, "(&s&s&s)", &name, &old_owner, &new_owner);
          (void)name;
          (void)old_owner;
          if (!self->active) return;
          ++self->generation;
          self->clear_registration();
          self->operation_in_flight = false;
          self->available = false;
          if (new_owner != nullptr && *new_owner != '\0') self->verify_version();
        },
        owner, [](gpointer data) { delete static_cast<std::shared_ptr<Impl>*>(data); });
    verify_version();
  }

  void shutdown() {
    if (!active) return;
    active = false;
    ++generation;
    operation_in_flight = false;
    if (cancellable != nullptr) g_cancellable_cancel(cancellable);
    clear_registration();
    if (connection != nullptr && name_owner_subscription != 0) {
      g_dbus_connection_signal_unsubscribe(connection, name_owner_subscription);
      name_owner_subscription = 0;
    }
    g_clear_object(&connection);
    g_clear_object(&cancellable);
  }

  void clear_registration() {
    ++registration_epoch;
    if (connection != nullptr && activation_subscription != 0) {
      g_dbus_connection_signal_unsubscribe(connection, activation_subscription);
      activation_subscription = 0;
    }
    registered = false;
    registered_id.clear();
  }

  void verify_version() {
    if (connection == nullptr || cancellable == nullptr || !active) return;
    auto* call = new VersionCall{shared_from_this(), generation};
    g_dbus_connection_call(
        connection, kCompanionName, kCompanionPath, kCompanionInterface, "Version",
        nullptr, G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE,
        kRpcTimeoutMilliseconds, cancellable,
        [](GObject* source, GAsyncResult* result, gpointer data) {
          std::unique_ptr<VersionCall> call(static_cast<VersionCall*>(data));
          const auto self = call->owner;
          g_autoptr(GError) error = nullptr;
          g_autoptr(GVariant) reply = g_dbus_connection_call_finish(
              G_DBUS_CONNECTION(source), result, &error);
          if (!self->active || call->generation != self->generation) return;
          guint version = 0;
          const bool available = reply != nullptr &&
              g_variant_is_of_type(reply, G_VARIANT_TYPE("(u)"));
          if (available) g_variant_get(reply, "(u)", &version);
          self->available = available && version == 1;
        },
        call);
  }

  void subscribe_activations() {
    if (connection == nullptr || activation_subscription != 0 || !active) return;
    auto* subscription =
        new ActivationSubscription{shared_from_this(), registration_epoch};
    activation_subscription = g_dbus_connection_signal_subscribe(
        connection, kCompanionName, kCompanionInterface, "Activated",
        kCompanionPath, nullptr, G_DBUS_SIGNAL_FLAGS_NONE,
        [](GDBusConnection*, const gchar*, const gchar*, const gchar*,
           const gchar*, GVariant* parameters, gpointer data) {
          const auto* subscription = static_cast<ActivationSubscription*>(data);
          const auto self = subscription->owner;
          const gchar* id = nullptr;
          g_variant_get(parameters, "(&s)", &id);
          if (self->active && self->registered &&
              subscription->registration_epoch == self->registration_epoch &&
              id != nullptr &&
              self->registered_id == id && self->activated != nullptr) {
            self->activated(id);
          }
        },
        subscription,
        [](gpointer data) { delete static_cast<ActivationSubscription*>(data); });
  }

  void register_shortcut(const ShortcutRequest& request,
                         std::function<void(ShortcutResult)> done) {
    if (!active || !available || connection == nullptr || cancellable == nullptr ||
        request.id.empty()) {
      done({false, {}, "The GNOME shortcut companion is unavailable."});
      return;
    }
    const std::string accelerator = gtk_accelerator(request);
    if (accelerator.empty()) {
      done({false, {}, "The selected shortcut has no GNOME accelerator."});
      return;
    }
    if (operation_in_flight) {
      done({false, {}, "A GNOME shortcut operation is already in progress."});
      return;
    }
    operation_in_flight = true;
    const guint operation_generation = generation;
    const std::string previous_id = registered_id;
    if (!previous_id.empty()) {
      dispatch_unregister(previous_id,
                          [self = shared_from_this(), request, accelerator,
                           operation_generation, done = std::move(done)](
                              bool removed) mutable {
                            if (!removed) {
                              self->finish_operation(operation_generation);
                              done({false, {}, "GNOME did not release the existing shortcut."});
                              return;
                            }
                            self->dispatch_register(request.id, accelerator, std::move(done));
                          });
      return;
    }
    dispatch_register(request.id, accelerator, std::move(done));
  }

  void dispatch_register(const std::string& id, const std::string& accelerator,
                         std::function<void(ShortcutResult)> done) {
    auto* call = new RegisterCall{shared_from_this(), generation, id, std::move(done)};
    g_dbus_connection_call(
        connection, kCompanionName, kCompanionPath, kCompanionInterface,
        "RegisterShortcut", g_variant_new("(ss)", id.c_str(), accelerator.c_str()),
        G_VARIANT_TYPE("(bs)"), G_DBUS_CALL_FLAGS_NONE, kRpcTimeoutMilliseconds,
        cancellable,
        [](GObject* source, GAsyncResult* result, gpointer data) {
          std::unique_ptr<RegisterCall> call(static_cast<RegisterCall*>(data));
          const auto self = call->owner;
          g_autoptr(GError) error = nullptr;
          g_autoptr(GVariant) reply = g_dbus_connection_call_finish(
              G_DBUS_CONNECTION(source), result, &error);
          gboolean registered = FALSE;
          const gchar* description = nullptr;
          if (self->active && call->generation == self->generation &&
              reply != nullptr && g_variant_is_of_type(reply, G_VARIANT_TYPE("(bs)"))) {
            g_variant_get(reply, "(b&s)", &registered, &description);
          }
          if (registered && description != nullptr && *description != '\0') {
            self->registered = true;
            self->registered_id = call->id;
            self->subscribe_activations();
            self->finish_operation(call->generation);
            call->done({true, description, {}});
            return;
          }
          self->finish_operation(call->generation);
          call->done({false, {}, "GNOME did not register the requested shortcut."});
        },
        call);
  }

  void unregister_shortcut(const std::string& id, std::function<void(bool)> done) {
    if (id.empty() || (registered && registered_id != id)) {
      done(false);
      return;
    }
    if (!registered) {
      done(true);
      return;
    }
    if (!active || connection == nullptr || cancellable == nullptr) {
      done(false);
      return;
    }
    if (operation_in_flight) {
      done(false);
      return;
    }
    operation_in_flight = true;
    const guint operation_generation = generation;
    dispatch_unregister(id, [self = shared_from_this(), operation_generation,
                            done = std::move(done)](bool removed) mutable {
      self->finish_operation(operation_generation);
      done(removed);
    });
  }

  void dispatch_unregister(const std::string& id, std::function<void(bool)> done) {
    auto* call = new UnregisterCall{shared_from_this(), generation, std::move(done)};
    g_dbus_connection_call(
        connection, kCompanionName, kCompanionPath, kCompanionInterface,
        "UnregisterShortcut", g_variant_new("(s)", id.c_str()),
        G_VARIANT_TYPE("(b)"), G_DBUS_CALL_FLAGS_NONE, kRpcTimeoutMilliseconds,
        cancellable,
        [](GObject* source, GAsyncResult* result, gpointer data) {
          std::unique_ptr<UnregisterCall> call(static_cast<UnregisterCall*>(data));
          const auto self = call->owner;
          g_autoptr(GError) error = nullptr;
          g_autoptr(GVariant) reply = g_dbus_connection_call_finish(
              G_DBUS_CONNECTION(source), result, &error);
          gboolean removed = FALSE;
          if (self->active && call->generation == self->generation &&
              reply != nullptr && g_variant_is_of_type(reply, G_VARIANT_TYPE("(b)"))) {
            g_variant_get(reply, "(b)", &removed);
          }
          if (removed) self->clear_registration();
          call->done(removed);
        },
        call);
  }

  void finish_operation(guint expected_generation) {
    if (expected_generation == generation) {
      operation_in_flight = false;
    }
  }

  std::function<void(const std::string&)> activated;
  GDBusConnection* connection = nullptr;
  GCancellable* cancellable = nullptr;
  guint name_owner_subscription = 0;
  guint activation_subscription = 0;
  guint generation = 0;
  guint registration_epoch = 0;
  bool active = true;
  bool available = false;
  bool registered = false;
  bool operation_in_flight = false;
  std::string registered_id;
};

LinuxGnomeShortcuts::LinuxGnomeShortcuts(
    std::function<void(const std::string&)> activated)
    : impl_(std::make_shared<Impl>(std::move(activated))) {
  impl_->start();
}

LinuxGnomeShortcuts::~LinuxGnomeShortcuts() { impl_->shutdown(); }

bool LinuxGnomeShortcuts::is_available() const { return impl_->available; }

bool LinuxGnomeShortcuts::shortcut_registered() const {
  return impl_->registered;
}

void LinuxGnomeShortcuts::register_shortcut(
    const ShortcutRequest& request, std::function<void(ShortcutResult)> done) {
  impl_->register_shortcut(request, std::move(done));
}

void LinuxGnomeShortcuts::unregister_shortcut(
    const std::string& id, std::function<void(bool)> done) {
  impl_->unregister_shortcut(id, std::move(done));
}
