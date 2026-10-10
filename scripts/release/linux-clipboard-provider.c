#define _GNU_SOURCE

#include <fcntl.h>
#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define MAX_OFFERS 16
#define MAX_PAYLOAD_BYTES (32 * 1024 * 1024)

typedef struct {
  gchar *mime;
  guchar *bytes;
  gsize length;
} Offer;

typedef struct {
  const gchar *activity_path;
  Offer offers[MAX_OFFERS];
  guint count;
} Provider;

static gboolean valid_application_id(const gchar *value) {
  if (value == NULL || *value == '\0' || strlen(value) > 255) return FALSE;
  for (const gchar *cursor = value; *cursor; cursor++) {
    if (!(g_ascii_isalnum(*cursor) || *cursor == '_' || *cursor == '-' || *cursor == '.')) return FALSE;
  }
  return TRUE;
}

static gboolean valid_mime(const gchar *value) {
  if (value == NULL || *value == '\0' || strlen(value) > 255) return FALSE;
  for (const gchar *cursor = value; *cursor; cursor++) {
    if (!(g_ascii_isalnum(*cursor) || strchr("!#$&^_.+-/;=", *cursor) != NULL)) return FALSE;
  }
  return TRUE;
}

static gboolean read_payload(const gchar *path, Offer *offer) {
  struct stat details;
  int descriptor = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (descriptor < 0 || fstat(descriptor, &details) != 0 || !S_ISREG(details.st_mode) || details.st_size < 0 || details.st_size > MAX_PAYLOAD_BYTES) {
    if (descriptor >= 0) close(descriptor);
    return FALSE;
  }
  offer->length = (gsize)details.st_size;
  offer->bytes = g_malloc(offer->length == 0 ? 1 : offer->length);
  gsize offset = 0;
  while (offset < offer->length) {
    ssize_t received = read(descriptor, offer->bytes + offset, offer->length - offset);
    if (received <= 0) {
      close(descriptor);
      g_free(offer->bytes);
      offer->bytes = NULL;
      return FALSE;
    }
    offset += (gsize)received;
  }
  close(descriptor);
  return TRUE;
}

static void record_request(const gchar *path, const gchar *mime) {
  int descriptor = open(path, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
  if (descriptor < 0) return;
  dprintf(descriptor, "{\"mime\":\"%s\"}\n", mime);
  fsync(descriptor);
  close(descriptor);
}

static void provide(GtkClipboard *clipboard, GtkSelectionData *selection, guint info, gpointer user_data) {
  (void)clipboard;
  Provider *provider = user_data;
  if (info >= provider->count) return;
  Offer *offer = &provider->offers[info];
  record_request(provider->activity_path, offer->mime);
  gtk_selection_data_set(selection, gdk_atom_intern(offer->mime, FALSE), 8, offer->bytes, (gint)offer->length);
}

static void clear(GtkClipboard *clipboard, gpointer user_data) {
  (void)clipboard;
  (void)user_data;
}

static gboolean stop(gpointer unused) {
  (void)unused;
  gtk_main_quit();
  return G_SOURCE_REMOVE;
}

static gboolean write_ready(const gchar *path) {
  int descriptor = open(path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
  if (descriptor < 0) return FALSE;
  gboolean okay = write(descriptor, "ready\n", 6) == 6 && fsync(descriptor) == 0;
  close(descriptor);
  return okay;
}

int main(int argc, char **argv) {
  Provider provider = {0};
  const gchar *application_id = NULL;
  const gchar *ready_path = NULL;
  guint hold_seconds = 0;
  for (int index = 1; index < argc;) {
    if (index + 1 < argc && strcmp(argv[index], "--application-id") == 0) {
      application_id = argv[index + 1];
      index += 2;
    } else if (index + 1 < argc && strcmp(argv[index], "--ready-file") == 0) {
      ready_path = argv[index + 1];
      index += 2;
    } else if (index + 1 < argc && strcmp(argv[index], "--activity-log") == 0) {
      provider.activity_path = argv[index + 1];
      index += 2;
    } else if (index + 1 < argc && strcmp(argv[index], "--hold-seconds") == 0) {
      gchar *end = NULL;
      unsigned long parsed = strtoul(argv[index + 1], &end, 10);
      if (*argv[index + 1] == '\0' || *end != '\0' || parsed < 1 || parsed > 60) return 2;
      hold_seconds = (guint)parsed;
      index += 2;
    } else if (index + 2 < argc && strcmp(argv[index], "--offer") == 0 && provider.count < MAX_OFFERS) {
      Offer *offer = &provider.offers[provider.count];
      if (!valid_mime(argv[index + 1]) || !read_payload(argv[index + 2], offer)) return 2;
      offer->mime = g_strdup(argv[index + 1]);
      provider.count++;
      index += 3;
    } else {
      return 2;
    }
  }
  if (!valid_application_id(application_id) || ready_path == NULL || provider.activity_path == NULL || hold_seconds == 0 || provider.count == 0) return 2;

  // GTK 3 Wayland derives the xdg app ID from g_get_prgname() during display
  // initialization. Set it before gtk_init so source attribution observes the
  // manifest's application ID instead of this helper's executable filename.
  g_set_prgname(application_id);
  // Keep the explicit X11 WM class used by the X11 attribution path.
  gdk_set_program_class(application_id);
  gtk_init(&argc, &argv);
  GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title(GTK_WINDOW(window), "CopyPaste qualification source");
  gtk_window_set_default_size(GTK_WINDOW(window), 240, 80);
  gtk_widget_show_all(window);

  GtkTargetEntry targets[MAX_OFFERS];
  for (guint index = 0; index < provider.count; index++) {
    targets[index] = (GtkTargetEntry){.target = provider.offers[index].mime, .flags = 0, .info = index};
  }
  GtkClipboard *clipboard = gtk_clipboard_get(GDK_SELECTION_CLIPBOARD);
  if (!gtk_clipboard_set_with_data(clipboard, targets, provider.count, provide, clear, &provider) || !write_ready(ready_path)) return 1;
  g_timeout_add_seconds(hold_seconds, stop, NULL);
  gtk_main();
  for (guint index = 0; index < provider.count; index++) {
    g_free(provider.offers[index].mime);
    g_free(provider.offers[index].bytes);
  }
  return 0;
}
