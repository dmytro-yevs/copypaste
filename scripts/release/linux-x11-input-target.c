// Private GTK target used only by Linux product qualification.
//
// The target owns a focused GtkEntry and records its current value. This lets
// the qualification driver observe the XTEST Ctrl+V sent by the packaged
// CopyPaste Quick Paste host without reading clipboard contents itself.

#include <gtk/gtk.h>

#include <string.h>

static const char kTitle[] = "CopyPaste Qualification Input Target";

struct State {
  const char* result_path;
};

void write_text(GtkEditable* editable, gpointer data) {
  const struct State* state = data;
  const char* text = gtk_entry_get_text(GTK_ENTRY(editable));
  GError* error = NULL;
  if (!g_file_set_contents(state->result_path, text, -1, &error)) {
    g_clear_error(&error);
  }
}

void activate(GtkApplication* application, gpointer data) {
  struct State* state = data;
  GtkWidget* window = gtk_application_window_new(application);
  gtk_window_set_title(GTK_WINDOW(window), kTitle);
  gtk_window_set_default_size(GTK_WINDOW(window), 480, 72);
  GtkWidget* entry = gtk_entry_new();
  gtk_container_add(GTK_CONTAINER(window), entry);
  g_signal_connect(entry, "changed", G_CALLBACK(write_text), state);
  gtk_widget_show_all(window);
  gtk_widget_grab_focus(entry);
}

int main(int argc, char** argv) {
  const char* ready_path = NULL;
  const char* result_path = NULL;
  for (int index = 1; index + 1 < argc; index += 2) {
    if (strcmp(argv[index], "--ready-file") == 0) ready_path = argv[index + 1];
    if (strcmp(argv[index], "--result-file") == 0) result_path = argv[index + 1];
  }
  if (ready_path == NULL || result_path == NULL) return 2;
  struct State state = {result_path};
  GError* error = NULL;
  if (!g_file_set_contents(ready_path, "ready\n", -1, &error)) {
    g_clear_error(&error);
    return 1;
  }
  GtkApplication* application = gtk_application_new(
      "org.copypaste.QualificationInputTarget", G_APPLICATION_NON_UNIQUE);
  g_signal_connect(application, "activate", G_CALLBACK(activate), &state);
  const int result = g_application_run(G_APPLICATION(application), 1, argv);
  g_object_unref(application);
  return result;
}
