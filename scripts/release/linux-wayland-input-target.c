// Private GTK target used only by Linux Wayland product qualification.
//
// It records a real portal-delivered paste and the widget focus transition so
// the release driver can prove that CopyPaste restored the original surface
// before sending Ctrl+V. It never reads the clipboard directly.

#include <gtk/gtk.h>

#include <string.h>
#include <unistd.h>

static const char kTitle[] = "CopyPaste Qualification Wayland Input Target";

struct State {
  const char* ready_path;
  const char* result_path;
  const char* marker;
  gboolean ready;
  gboolean marker_received;
  guint focus_events;
};

static void write_state(const struct State* state) {
  gchar* value = g_strdup_printf(
      "{\"focus_events\":%u,\"marker_received\":%s}\n",
      state->focus_events, state->marker_received ? "true" : "false");
  GError* error = NULL;
  if (!g_file_set_contents(state->result_path, value, -1, &error)) {
    g_clear_error(&error);
  }
  g_free(value);
}

static void write_result(GtkEditable* editable, gpointer data) {
  struct State* state = data;
  if (g_strcmp0(gtk_entry_get_text(GTK_ENTRY(editable)), state->marker) == 0) {
    state->marker_received = TRUE;
  }
  write_state(state);
}

static gboolean focused(GtkWidget* widget, GdkEventFocus* event, gpointer data) {
  (void)widget;
  (void)event;
  struct State* state = data;
  state->focus_events += 1;
  write_state(state);
  if (state->ready) return FALSE;
  GError* error = NULL;
  gchar* ready = g_strdup_printf("focused pid=%ld\n", (long)getpid());
  if (g_file_set_contents(state->ready_path, ready, -1, &error)) {
    state->ready = TRUE;
  } else {
    g_clear_error(&error);
  }
  g_free(ready);
  return FALSE;
}

static void activate(GtkApplication* application, gpointer data) {
  struct State* state = data;
  GtkWidget* window = gtk_application_window_new(application);
  gtk_window_set_title(GTK_WINDOW(window), kTitle);
  gtk_window_set_default_size(GTK_WINDOW(window), 480, 72);
  GtkWidget* entry = gtk_entry_new();
  gtk_container_add(GTK_CONTAINER(window), entry);
  g_signal_connect(entry, "changed", G_CALLBACK(write_result), state);
  g_signal_connect(entry, "focus-in-event", G_CALLBACK(focused), state);
  gtk_widget_show_all(window);
  gtk_widget_grab_focus(entry);
}

int main(int argc, char** argv) {
  const char* ready_path = NULL;
  const char* result_path = NULL;
  const char* marker = "copypaste-wayland-quick-paste-fixture";
  for (int index = 1; index + 1 < argc; index += 2) {
    if (strcmp(argv[index], "--ready-file") == 0) ready_path = argv[index + 1];
    if (strcmp(argv[index], "--result-file") == 0) result_path = argv[index + 1];
    if (strcmp(argv[index], "--marker") == 0) marker = argv[index + 1];
  }
  if (ready_path == NULL || result_path == NULL || strlen(marker) == 0 || strlen(marker) > 512) return 2;
  for (const char* value = marker; *value != '\0'; ++value) {
    if ((unsigned char)*value < 0x20 || (unsigned char)*value > 0x7e) return 2;
  }
  struct State state = {ready_path, result_path, marker, FALSE, FALSE, 0};
  GtkApplication* application = gtk_application_new(
      "org.copypaste.QualificationWaylandInputTarget", G_APPLICATION_NON_UNIQUE);
  g_signal_connect(application, "activate", G_CALLBACK(activate), &state);
  const int result = g_application_run(G_APPLICATION(application), 1, argv);
  g_object_unref(application);
  return result;
}
