#include "my_application.h"

#include <flutter_linux/flutter_linux.h>

#if defined(GDK_WINDOWING_X11)
#include <gdk/gdkx.h>
#endif

#include <cerrno>
#include <cstdint>
#include <cstring>
#include <cstdlib>
#include <limits.h>
#include <limits>

#include "flutter/generated_plugin_registrant.h"
#include "linux_host_channels.h"
#include "linux_quick_paste_window.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  gboolean quick_paste;
  GtkWindow* root_window;
  GQueue* pending_pairing_uris;
  gboolean flutter_ready;
  guint32 x11_activation_time;
  gboolean x11_activation_time_invalid;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

namespace {

constexpr gsize kMaxPairingUriBytes = 8192;
constexpr guint kMaxPendingPairingUris = 8;

bool is_runtime_endpoint(const gchar* endpoint) {
  return endpoint != nullptr && *endpoint != '\0' &&
         strnlen(endpoint, PATH_MAX) < PATH_MAX && g_path_is_absolute(endpoint);
}

bool is_pairing_uri(const gchar* argument) {
  if (argument == nullptr ||
      strnlen(argument, kMaxPairingUriBytes + 1) > kMaxPairingUriBytes) {
    return false;
  }
  return g_str_has_prefix(argument, "copypaste://pair/v1?") ||
         g_str_has_prefix(argument, "copypaste://pair/v2?");
}

bool parse_x11_activation_time(const gchar* value, guint32* activation_time) {
  if (value == nullptr || *value == '\0' || activation_time == nullptr) return false;
  for (const gchar* character = value; *character != '\0'; ++character) {
    if (!g_ascii_isdigit(*character)) return false;
  }
  errno = 0;
  char* end = nullptr;
  const unsigned long parsed = std::strtoul(value, &end, 10);
  if (errno != 0 || end == value || *end != '\0' || parsed == GDK_CURRENT_TIME ||
      parsed > std::numeric_limits<guint32>::max()) {
    return false;
  }
  *activation_time = static_cast<guint32>(parsed);
  return true;
}

void dispatch_pending_pairing_uris(MyApplication* self) {
  if (!self->flutter_ready) return;
  while (!g_queue_is_empty(self->pending_pairing_uris)) {
    gchar* uri = static_cast<gchar*>(g_queue_pop_head(self->pending_pairing_uris));
    deliver_linux_pairing_uri(uri);
    g_free(uri);
  }
}

void queue_pairing_uri(MyApplication* self, const gchar* uri) {
  if (!is_pairing_uri(uri)) return;
  if (g_queue_get_length(self->pending_pairing_uris) >= kMaxPendingPairingUris) {
    g_warning("Ignoring pairing link because the pending queue is full.");
    return;
  }
  g_queue_push_tail(self->pending_pairing_uris, g_strdup(uri));
  dispatch_pending_pairing_uris(self);
}

void set_quick_paste_context(MyApplication* self, const gchar* argument) {
  if (g_str_has_prefix(argument, "--copypaste-transaction=")) {
    g_object_set_data_full(G_OBJECT(self), "copypaste-transaction",
                           g_strdup(argument + strlen("--copypaste-transaction=")),
                           g_free);
  } else if (g_str_has_prefix(argument, "--copypaste-presentation-id=")) {
    g_object_set_data_full(
        G_OBJECT(self), "copypaste-presentation-id",
        g_strdup(argument + strlen("--copypaste-presentation-id=")), g_free);
  } else if (g_str_has_prefix(argument, "--copypaste-x11-window=")) {
    g_object_set_data_full(G_OBJECT(self), "copypaste-x11-window",
                           g_strdup(argument + strlen("--copypaste-x11-window=")),
                           g_free);
  } else if (g_str_has_prefix(argument, "--copypaste-x11-activation-time=")) {
    guint32 activation_time = GDK_CURRENT_TIME;
    const gchar* value = argument + strlen("--copypaste-x11-activation-time=");
    if (self->x11_activation_time != GDK_CURRENT_TIME ||
        self->x11_activation_time_invalid ||
        !parse_x11_activation_time(value, &activation_time)) {
      self->x11_activation_time = GDK_CURRENT_TIME;
      self->x11_activation_time_invalid = TRUE;
    } else {
      self->x11_activation_time = activation_time;
    }
  } else if (g_str_has_prefix(argument, "--copypaste-runtime-endpoint=")) {
    const gchar* endpoint = argument + strlen("--copypaste-runtime-endpoint=");
    if (!is_runtime_endpoint(endpoint) ||
        g_object_get_data(G_OBJECT(self), "copypaste-runtime-endpoint") != nullptr) {
      g_object_set_data(G_OBJECT(self), "copypaste-runtime-endpoint-invalid",
                        GINT_TO_POINTER(1));
    } else {
      g_object_set_data_full(G_OBJECT(self), "copypaste-runtime-endpoint",
                             g_strdup(endpoint), g_free);
    }
  }
}

void first_frame_cb(MyApplication* self, FlView* view) {
  GtkWidget* window = gtk_widget_get_toplevel(GTK_WIDGET(view));
  gtk_widget_show(window);
  if (self->quick_paste) {
    // Preserve the compositor's initial monitor/position. Clamp only the size
    // after the surface exists; inspector changes can clamp its current frame.
    resize_linux_quick_paste_window(GTK_WINDOW(window), false, false);
#if defined(GDK_WINDOWING_X11)
    if (!self->x11_activation_time_invalid &&
        self->x11_activation_time != GDK_CURRENT_TIME &&
        GDK_IS_X11_DISPLAY(gtk_widget_get_display(window))) {
      gtk_window_present_with_time(GTK_WINDOW(window), self->x11_activation_time);
    }
#endif
  }
  self->flutter_ready = TRUE;
  dispatch_pending_pairing_uris(self);
}

}  // namespace

static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  if (!self->quick_paste && self->root_window != nullptr) {
    gtk_window_present(self->root_window);
    return;
  }
  const bool quick_paste = self->quick_paste;
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));
  gtk_window_set_default_size(window, 1280, 720);
  gtk_window_set_title(window, quick_paste ? "CopyPaste Quick Paste" : "CopyPaste");
  if (quick_paste) {
    configure_linux_quick_paste_window(window, [] {
      close_linux_quick_paste_context();
    });
  }

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(project,
                                                self->dart_entrypoint_arguments);
  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  gdk_rgba_parse(&background_color, quick_paste ? "transparent" : "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));
  if (!quick_paste) {
    self->root_window = window;
    g_object_add_weak_pointer(G_OBJECT(window),
                              reinterpret_cast<gpointer*>(&self->root_window));
  }
  register_linux_host_channels(fl_engine_get_binary_messenger(fl_view_get_engine(view)),
                               GTK_APPLICATION(self));
  gtk_widget_grab_focus(GTK_WIDGET(view));
}

static gint my_application_command_line(GApplication* application,
                                        GApplicationCommandLine* command_line) {
  MyApplication* self = MY_APPLICATION(application);
  gint argc = 0;
  g_auto(GStrv) arguments =
      g_application_command_line_get_arguments(command_line, &argc);
  if (arguments == nullptr || argc == 0) return 1;

  if (self->quick_paste) {
    g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
    self->dart_entrypoint_arguments = g_strdupv(arguments + 1);
    for (gint index = 1; index < argc; ++index) {
      set_quick_paste_context(self, arguments[index]);
    }
  } else {
    for (gint index = 1; index < argc; ++index) {
      queue_pairing_uri(self, arguments[index]);
    }
    if (self->dart_entrypoint_arguments == nullptr) {
      self->dart_entrypoint_arguments = g_strdupv(arguments + 1);
    }
  }
  g_application_activate(application);
  return 0;
}

static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  if (self->pending_pairing_uris != nullptr) {
    g_queue_free_full(self->pending_pairing_uris, g_free);
    self->pending_pairing_uris = nullptr;
  }
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->command_line = my_application_command_line;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {
  self->pending_pairing_uris = g_queue_new();
}

MyApplication* my_application_new(gboolean quick_paste) {
  g_set_prgname(APPLICATION_ID);
  GApplicationFlags flags = G_APPLICATION_HANDLES_COMMAND_LINE;
  if (quick_paste) {
    flags = static_cast<GApplicationFlags>(flags | G_APPLICATION_NON_UNIQUE);
  }
  MyApplication* application = MY_APPLICATION(g_object_new(
      my_application_get_type(), "application-id", APPLICATION_ID, "flags", flags,
      nullptr));
  application->quick_paste = quick_paste;
  if (quick_paste) {
    // Native host channels use this existing process-scoping marker to avoid
    // owning root-only D-Bus services from the transient child.
    g_object_set_data(G_OBJECT(application), "copypaste-quick-paste",
                      GINT_TO_POINTER(1));
  }
  return application;
}
