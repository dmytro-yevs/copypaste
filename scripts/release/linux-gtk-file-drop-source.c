// Private GTK drag source used only by Linux product qualification.
//
// It offers a real text/uri-list selection to the installed History surface.
// A separate, permissioned desktop-input actor must move the actual pointer
// from this source into the installed application; this program does not
// synthesize input or claim that the destination accepted a drop.

#include <fcntl.h>
#include <gtk/gtk.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define MAX_URI_LIST_BYTES 8192

typedef struct {
  const char* activity_path;
  const char* start_path;
  gchar* uri_list;
  gsize uri_list_size;
  gboolean started;
} State;

static gboolean regular_file(const char* path, gsize maximum) {
  struct stat details;
  return path != NULL && lstat(path, &details) == 0 && S_ISREG(details.st_mode) &&
      !S_ISLNK(details.st_mode) && details.st_size > 0 && (gsize)details.st_size <= maximum;
}

static gboolean write_once(const char* path, const char* value) {
  int descriptor = open(path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
  if (descriptor < 0) return FALSE;
  const size_t length = strlen(value);
  const gboolean success = write(descriptor, value, length) == (ssize_t)length && fsync(descriptor) == 0;
  close(descriptor);
  return success;
}

static void activity(const State* state, const char* event) {
  int descriptor = open(state->activity_path, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
  if (descriptor < 0) return;
  dprintf(descriptor, "{\"event\":\"%s\"}\n", event);
  fsync(descriptor);
  close(descriptor);
}

static void provide(GtkWidget* widget, GdkDragContext* context, GtkSelectionData* selection,
                    guint info, guint time, gpointer data) {
  (void)widget;
  (void)context;
  (void)info;
  (void)time;
  const State* state = data;
  gtk_selection_data_set(selection, gdk_atom_intern("text/uri-list", FALSE), 8,
                         (const guchar*)state->uri_list, (gint)state->uri_list_size);
  activity(state, "uri_list_requested");
}

static void end_drag(GtkWidget* widget, GdkDragContext* context, gpointer data) {
  (void)widget;
  State* state = data;
  activity(state, gdk_drag_context_get_selected_action(context) == GDK_ACTION_COPY ? "drag_completed" : "drag_not_accepted");
}

static gboolean start_drag(gpointer data) {
  State* state = data;
  if (state->started || !g_file_test(state->start_path, G_FILE_TEST_IS_REGULAR)) return G_SOURCE_CONTINUE;
  state->started = TRUE;
  GtkWidget* source = g_object_get_data(G_OBJECT(g_application_get_default()), "source-button");
  GtkTargetEntry target = {(gchar*)"text/uri-list", 0, 0};
  GtkTargetList* targets = gtk_target_list_new(&target, 1);
  GdkDragContext* context = gtk_drag_begin_with_coordinates(source, targets, GDK_ACTION_COPY, 1, NULL, -1, -1);
  gtk_target_list_unref(targets);
  if (context == NULL) {
    activity(state, "drag_begin_failed");
    return G_SOURCE_REMOVE;
  }
  activity(state, "drag_started");
  return G_SOURCE_REMOVE;
}

static void activate(GtkApplication* application, gpointer data) {
  State* state = data;
  GtkWidget* window = gtk_application_window_new(application);
  gtk_window_set_title(GTK_WINDOW(window), "CopyPaste Qualification File Drop Source");
  gtk_window_set_default_size(GTK_WINDOW(window), 320, 96);
  GtkWidget* source = gtk_button_new_with_label("Drag qualification file");
  gtk_drag_source_set(source, GDK_BUTTON1_MASK, NULL, 0, GDK_ACTION_COPY);
  GtkTargetEntry target = {(gchar*)"text/uri-list", 0, 0};
  gtk_drag_source_set_target_list(source, gtk_target_list_new(&target, 1));
  g_signal_connect(source, "drag-data-get", G_CALLBACK(provide), state);
  g_signal_connect(source, "drag-end", G_CALLBACK(end_drag), state);
  gtk_container_add(GTK_CONTAINER(window), source);
  g_object_set_data(G_OBJECT(application), "source-button", source);
  gtk_widget_show_all(window);
  g_timeout_add(50, start_drag, state);
}

int main(int argc, char** argv) {
  const char* ready_path = NULL;
  const char* uri_list_path = NULL;
  State state = {0};
  for (int index = 1; index + 1 < argc; index += 2) {
    if (strcmp(argv[index], "--ready-file") == 0) ready_path = argv[index + 1];
    else if (strcmp(argv[index], "--start-file") == 0) state.start_path = argv[index + 1];
    else if (strcmp(argv[index], "--activity-log") == 0) state.activity_path = argv[index + 1];
    else if (strcmp(argv[index], "--uri-list-file") == 0) uri_list_path = argv[index + 1];
    else return 2;
  }
  if (ready_path == NULL || state.start_path == NULL || state.activity_path == NULL ||
      !regular_file(uri_list_path, MAX_URI_LIST_BYTES)) return 2;
  GError* error = NULL;
  if (!g_file_get_contents(uri_list_path, &state.uri_list, &state.uri_list_size, &error)) {
    g_clear_error(&error);
    return 1;
  }
  if (!g_utf8_validate(state.uri_list, (gssize)state.uri_list_size, NULL) ||
      !g_str_has_prefix(state.uri_list, "file://")) {
    g_free(state.uri_list);
    return 2;
  }
  if (!write_once(ready_path, "ready\n")) {
    g_free(state.uri_list);
    return 1;
  }
  GtkApplication* application = gtk_application_new(
      "org.copypaste.QualificationFileDropSource", G_APPLICATION_NON_UNIQUE);
  g_signal_connect(application, "activate", G_CALLBACK(activate), &state);
  const int result = g_application_run(G_APPLICATION(application), 1, argv);
  g_object_unref(application);
  g_free(state.uri_list);
  return result;
}
