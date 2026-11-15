#include "linux_window_material.h"

#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <gdk/gdkx.h>

#include <initializer_list>

namespace {

constexpr char kChannel[] = "com.copypaste.app/window_material";
constexpr char kBlurAtom[] = "_KDE_NET_WM_BLUR_BEHIND_REGION";
constexpr char kStateKey[] = "copypaste-window-material";
constexpr char kPreferDark[] = "gtk-application-prefer-dark-theme";

// The session's own preference, restored when the app follows the system.
int g_system_prefers_dark = -1;

// GTK draws the window frame, so the frame follows the app theme.
void set_appearance(const gchar* mode) {
  GtkSettings* settings = gtk_settings_get_default();
  if (settings == nullptr) return;
  if (g_system_prefers_dark < 0) {
    gboolean dark = FALSE;
    g_object_get(settings, kPreferDark, &dark, nullptr);
    g_system_prefers_dark = dark ? 1 : 0;
  }
  const gboolean dark = g_strcmp0(mode, "dark") == 0    ? TRUE
                        : g_strcmp0(mode, "light") == 0 ? FALSE
                                                       : g_system_prefers_dark;
  g_object_set(settings, kPreferDark, dark, nullptr);
}

constexpr char kAccentChannel[] = "com.copypaste.app/system_accent";
constexpr char kAppearanceNamespace[] = "org.freedesktop.appearance";
constexpr char kAccentKey[] = "accent-color";

struct WindowMaterialState {
  GtkWindow* window;
  FlMethodChannel* channel;
  bool supported;
  bool enabled;
  bool active;
  FlMethodChannel* accent_channel;
  GDBusProxy* settings_portal;
};

// The portal's accent is an (ddd) sRGB triple; a component outside 0..1
// means the user chose none.
FlValue* accent_value(GVariant* rgb) {
  if (rgb == nullptr || !g_variant_is_of_type(rgb, G_VARIANT_TYPE("(ddd)"))) {
    return fl_value_new_null();
  }
  double red = 0, green = 0, blue = 0;
  g_variant_get(rgb, "(ddd)", &red, &green, &blue);
  for (const double component : {red, green, blue}) {
    if (component < 0 || component > 1) return fl_value_new_null();
  }
  auto channel = [](double value) {
    return static_cast<int64_t>(value * 255 + 0.5);
  };
  return fl_value_new_int(0xFF000000LL | channel(red) << 16 |
                          channel(green) << 8 | channel(blue));
}

void on_accent_read(GObject* source, GAsyncResult* result, gpointer data) {
  g_autoptr(FlMethodCall) call = FL_METHOD_CALL(data);
  g_autoptr(GVariant) reply =
      g_dbus_proxy_call_finish(G_DBUS_PROXY(source), result, nullptr);
  g_autoptr(GVariant) value = nullptr;
  if (reply != nullptr) g_variant_get(reply, "(v)", &value);
  g_autoptr(FlValue) accent = accent_value(value);
  fl_method_call_respond_success(call, accent, nullptr);
}

void handle_accent_call(FlMethodChannel*, FlMethodCall* call,
                        gpointer user_data) {
  auto* state = static_cast<WindowMaterialState*>(user_data);
  if (g_strcmp0(fl_method_call_get_name(call), "read") != 0) {
    fl_method_call_respond_not_implemented(call, nullptr);
    return;
  }
  if (state->settings_portal == nullptr) {
    g_autoptr(FlValue) none = fl_value_new_null();
    fl_method_call_respond_success(call, none, nullptr);
    return;
  }
  g_dbus_proxy_call(state->settings_portal, "ReadOne",
                    g_variant_new("(ss)", kAppearanceNamespace, kAccentKey),
                    G_DBUS_CALL_FLAGS_NONE, -1, nullptr, on_accent_read,
                    g_object_ref(call));
}

void on_portal_signal(GDBusProxy*, const gchar*, const gchar* signal,
                      GVariant* parameters, gpointer user_data) {
  if (g_strcmp0(signal, "SettingChanged") != 0) return;
  const gchar* name_space = nullptr;
  const gchar* key = nullptr;
  g_autoptr(GVariant) value = nullptr;
  g_variant_get(parameters, "(&s&sv)", &name_space, &key, &value);
  if (g_strcmp0(name_space, kAppearanceNamespace) != 0 ||
      g_strcmp0(key, kAccentKey) != 0) {
    return;
  }
  auto* state = static_cast<WindowMaterialState*>(user_data);
  g_autoptr(FlValue) accent = accent_value(value);
  fl_method_channel_invoke_method(state->accent_channel, "changed", accent,
                                  nullptr, nullptr, nullptr);
}

// KWin's blur effect announces itself with this property on the root window.
bool blur_available(GdkScreen* screen) {
  if (screen == nullptr || !gdk_screen_is_composited(screen)) return false;
  GdkDisplay* display = gdk_screen_get_display(screen);
  if (!GDK_IS_X11_DISPLAY(display)) return false;
  Display* xdisplay = gdk_x11_display_get_xdisplay(display);
  const Atom atom = XInternAtom(xdisplay, kBlurAtom, True);
  if (atom == None) return false;
  int count = 0;
  Atom* properties = XListProperties(
      xdisplay, gdk_x11_window_get_xid(gdk_screen_get_root_window(screen)),
      &count);
  bool found = false;
  for (int index = 0; index < count && !found; ++index) {
    found = properties[index] == atom;
  }
  if (properties != nullptr) XFree(properties);
  return found;
}

// An empty region asks KWin to blur the whole window.
bool set_blur(GtkWindow* window, bool blur) {
  GdkWindow* surface = gtk_widget_get_window(GTK_WIDGET(window));
  if (surface == nullptr || !GDK_IS_X11_WINDOW(surface)) return false;
  Display* xdisplay = GDK_WINDOW_XDISPLAY(surface);
  const Window xid = GDK_WINDOW_XID(surface);
  const Atom atom = XInternAtom(xdisplay, kBlurAtom, False);
  if (blur) {
    XChangeProperty(xdisplay, xid, atom, XA_CARDINAL, 32, PropModeReplace,
                    nullptr, 0);
  } else {
    XDeleteProperty(xdisplay, xid, atom);
  }
  XFlush(xdisplay);
  return true;
}

FlValue* apply(WindowMaterialState* state) {
  const bool wanted = state->supported && state->enabled;
  if (wanted != state->active && set_blur(state->window, wanted)) {
    state->active = wanted;
  }
  FlValue* result = fl_value_new_map();
  fl_value_set_string_take(result, "material",
                           fl_value_new_string(state->active ? "blur" : "none"));
  fl_value_set_string_take(result, "supported",
                           fl_value_new_bool(state->supported));
  // Neither KDE nor GNOME exposes a reduced-transparency setting.
  fl_value_set_string_take(result, "reducedTransparency",
                           fl_value_new_bool(false));
  return result;
}

void respond(FlMethodCall* call, FlMethodResponse* response) {
  g_autoptr(FlMethodResponse) owned = response;
  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond(call, owned, &error)) {
    g_warning("Failed to answer window material call: %s", error->message);
  }
}

void handle_method_call(FlMethodChannel*, FlMethodCall* call,
                        gpointer user_data) {
  auto* state = static_cast<WindowMaterialState*>(user_data);
  if (g_strcmp0(fl_method_call_get_name(call), "setAppearance") == 0) {
    FlValue* args = fl_method_call_get_args(call);
    FlValue* mode =
        args != nullptr && fl_value_get_type(args) == FL_VALUE_TYPE_MAP
            ? fl_value_lookup_string(args, "mode")
            : nullptr;
    if (mode == nullptr || fl_value_get_type(mode) != FL_VALUE_TYPE_STRING) {
      respond(call, FL_METHOD_RESPONSE(fl_method_error_response_new(
                        "invalid_arguments", nullptr, nullptr)));
      return;
    }
    set_appearance(fl_value_get_string(mode));
    respond(call, FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr)));
    return;
  }
  if (g_strcmp0(fl_method_call_get_name(call), "setEnabled") != 0) {
    respond(call, FL_METHOD_RESPONSE(fl_method_not_implemented_response_new()));
    return;
  }
  FlValue* args = fl_method_call_get_args(call);
  FlValue* enabled = args != nullptr && fl_value_get_type(args) == FL_VALUE_TYPE_MAP
                         ? fl_value_lookup_string(args, "enabled")
                         : nullptr;
  if (enabled == nullptr || fl_value_get_type(enabled) != FL_VALUE_TYPE_BOOL) {
    respond(call, FL_METHOD_RESPONSE(fl_method_error_response_new(
                      "invalid_arguments", nullptr, nullptr)));
    return;
  }
  state->enabled = fl_value_get_bool(enabled);
  g_autoptr(FlValue) result = apply(state);
  respond(call, FL_METHOD_RESPONSE(fl_method_success_response_new(result)));
}

void free_state(gpointer data) {
  auto* state = static_cast<WindowMaterialState*>(data);
  fl_method_channel_set_method_call_handler(state->channel, nullptr, nullptr,
                                            nullptr);
  fl_method_channel_set_method_call_handler(state->accent_channel, nullptr,
                                            nullptr, nullptr);
  if (state->settings_portal != nullptr) {
    g_signal_handlers_disconnect_by_data(state->settings_portal, state);
    g_object_unref(state->settings_portal);
  }
  g_object_unref(state->accent_channel);
  g_object_unref(state->channel);
  delete state;
}

}  // namespace

bool prepare_linux_window_material(GtkWindow* window) {
  GtkWidget* widget = GTK_WIDGET(window);
  GdkScreen* screen = gtk_widget_get_screen(widget);
  if (!blur_available(screen)) return false;
  GdkVisual* visual = gdk_screen_get_rgba_visual(screen);
  if (visual == nullptr) return false;
  gtk_widget_set_visual(widget, visual);
  gtk_widget_set_app_paintable(widget, TRUE);
  return true;
}

void register_linux_window_material(FlBinaryMessenger* messenger,
                                    GtkWindow* window, bool supported) {
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  auto* state = new WindowMaterialState{
      window,
      fl_method_channel_new(messenger, kChannel, FL_METHOD_CODEC(codec)),
      supported,
      false,
      false,
      fl_method_channel_new(messenger, kAccentChannel, FL_METHOD_CODEC(codec)),
      // Sessions without the portal have no accent to follow.
      g_dbus_proxy_new_for_bus_sync(
          G_BUS_TYPE_SESSION,
          static_cast<GDBusProxyFlags>(G_DBUS_PROXY_FLAGS_DO_NOT_LOAD_PROPERTIES |
                                       G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START),
          nullptr, "org.freedesktop.portal.Desktop",
          "/org/freedesktop/portal/desktop", "org.freedesktop.portal.Settings",
          nullptr, nullptr),
  };
  fl_method_channel_set_method_call_handler(state->channel, handle_method_call,
                                            state, nullptr);
  fl_method_channel_set_method_call_handler(state->accent_channel,
                                            handle_accent_call, state, nullptr);
  if (state->settings_portal != nullptr) {
    g_signal_connect(state->settings_portal, "g-signal",
                     G_CALLBACK(on_portal_signal), state);
  }
  g_object_set_data_full(G_OBJECT(window), kStateKey, state, free_state);
}
