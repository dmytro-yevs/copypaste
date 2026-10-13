#define _GNU_SOURCE

#include <fcntl.h>
#include <gtk/gtk.h>
#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#endif
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#include <X11/Xlib.h>
#include <X11/extensions/XRes.h>
#endif
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
  const gchar *ready_path;
  const gchar *request_audit_path;
  int request_audit_descriptor;
  gboolean request_audit_filter_installed;
  Offer offers[MAX_OFFERS];
  guint count;
  guint hold_seconds;
  gboolean require_focus;
  gboolean selection_scheduled;
  gboolean ready;
  gboolean failed;
} Provider;

static gboolean write_ready(const gchar *path);

static gboolean write_all(int descriptor, const gchar *bytes, gsize length) {
  gsize offset = 0;
  while (offset < length) {
    ssize_t written = write(descriptor, bytes + offset, length - offset);
    if (written <= 0) return FALSE;
    offset += (gsize)written;
  }
  return TRUE;
}

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

static gboolean prepare_request_audit(Provider *provider) {
  int descriptor = open(provider->request_audit_path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
  if (descriptor < 0) return FALSE;
  struct stat details;
  if (fstat(descriptor, &details) != 0 || !S_ISREG(details.st_mode) || (details.st_mode & 0077) != 0) {
    close(descriptor);
    return FALSE;
  }
  provider->request_audit_descriptor = descriptor;
  return TRUE;
}

#ifdef GDK_WINDOWING_X11
typedef struct {
  Display *display;
  gboolean saw_error;
  int (*previous_handler)(Display *, XErrorEvent *);
} XErrorScope;

static XErrorScope *x_error_scope;

static int record_x_error(Display *display, XErrorEvent *event) {
  if (x_error_scope != NULL && x_error_scope->display == display) {
    x_error_scope->saw_error = TRUE;
    return 0;
  }
  if (x_error_scope != NULL && x_error_scope->previous_handler != NULL) {
    return x_error_scope->previous_handler(display, event);
  }
  return 0;
}

static void begin_x_error_scope(XErrorScope *scope, Display *display) {
  scope->display = display;
  scope->saw_error = FALSE;
  x_error_scope = scope;
  scope->previous_handler = XSetErrorHandler(record_x_error);
}

static gboolean finish_x_error_scope(XErrorScope *scope) {
  XSync(scope->display, False);
  XSetErrorHandler(scope->previous_handler);
  x_error_scope = NULL;
  return scope->saw_error;
}

static gchar *selection_target(Display *display, Atom target, const gchar **error) {
  XErrorScope scope = {.display = NULL, .saw_error = FALSE, .previous_handler = NULL};
  begin_x_error_scope(&scope, display);
  gchar *name = XGetAtomName(display, target);
  if (finish_x_error_scope(&scope) || name == NULL) {
    if (name != NULL) XFree(name);
    *error = "x_target_query_failed";
    return NULL;
  }
  return name;
}

static pid_t requestor_pid(Display *display, Window requestor, const gchar **error) {
  XResClientIdSpec specification = {.client = requestor, .mask = XRES_CLIENT_ID_PID_MASK};
  XResClientIdValue *identities = NULL;
  long count = 0;
  XErrorScope scope = {.display = NULL, .saw_error = FALSE, .previous_handler = NULL};
  begin_x_error_scope(&scope, display);
  Status status = XResQueryClientIds(display, 1, &specification, &count, &identities);
  if (finish_x_error_scope(&scope) || status != Success) {
    if (identities != NULL) XResClientIdsDestroy(count, identities);
    *error = "xres_query_failed";
    return -1;
  }
  for (long index = 0; index < count; index++) {
    if (XResGetClientIdType(&identities[index]) == XRES_CLIENT_ID_PID) {
      pid_t pid = XResGetClientPid(&identities[index]);
      XResClientIdsDestroy(count, identities);
      if (pid > 0) return pid;
      *error = "xres_pid_unavailable";
      return -1;
    }
  }
  if (identities != NULL) XResClientIdsDestroy(count, identities);
  *error = "xres_pid_unavailable";
  return -1;
}

static void append_json_string(GString *record, const gchar *value) {
  g_string_append_c(record, '"');
  for (const guchar *cursor = (const guchar *)value; *cursor; cursor++) {
    if (*cursor == '"' || *cursor == '\\') {
      g_string_append_c(record, '\\');
      g_string_append_c(record, (gchar)*cursor);
    } else if (*cursor >= 0x20 && *cursor <= 0x7e) {
      g_string_append_c(record, (gchar)*cursor);
    } else {
      g_string_append_printf(record, "\\u%04x", (unsigned int)*cursor);
    }
  }
  g_string_append_c(record, '"');
}

static GdkFilterReturn audit_selection_request(GdkXEvent *native_event, GdkEvent *event, gpointer user_data) {
  (void)event;
  Provider *provider = user_data;
  XEvent *xevent = native_event;
  if (xevent->type != SelectionRequest || provider->request_audit_descriptor < 0) return GDK_FILTER_CONTINUE;
  XSelectionRequestEvent *request = &xevent->xselectionrequest;
  const gchar *error = NULL;
  gchar *target = selection_target(request->display, request->target, &error);
  pid_t pid = error == NULL ? requestor_pid(request->display, request->requestor, &error) : -1;
  GString *record = g_string_new("{\"target\":");
  append_json_string(record, target == NULL ? "unknown" : target);
  if (pid > 0) {
    g_string_append_printf(record, ",\"requestor_pid\":%ld,\"error\":null}\n", (long)pid);
  } else {
    g_string_append(record, ",\"requestor_pid\":null,\"error\":");
    append_json_string(record, error == NULL ? "xres_pid_unavailable" : error);
    g_string_append(record, "}\n");
  }
  if (!write_all(provider->request_audit_descriptor, record->str, record->len) || fsync(provider->request_audit_descriptor) != 0) {
    provider->failed = TRUE;
    gtk_main_quit();
  }
  g_string_free(record, TRUE);
  if (target != NULL) XFree(target);
  return GDK_FILTER_CONTINUE;
}
#endif

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

static gboolean establish_clipboard(Provider *provider, GdkDisplay *display) {
  GtkTargetEntry targets[MAX_OFFERS];
  for (guint index = 0; index < provider->count; index++) {
    targets[index] = (GtkTargetEntry){.target = provider->offers[index].mime, .flags = 0, .info = index};
  }
  GtkClipboard *clipboard = gtk_clipboard_get(GDK_SELECTION_CLIPBOARD);
  if (!gtk_clipboard_set_with_data(clipboard, targets, provider->count, provide, clear, provider)) {
    provider->failed = TRUE;
    return FALSE;
  }

  // GTK3 passes the latest Wayland input serial to set_selection.  This idle
  // task runs after focus delivery has recorded that serial for the mapped
  // source window.
  gdk_display_flush(display);
  gdk_display_sync(display);
  if (!write_ready(provider->ready_path)) {
    provider->failed = TRUE;
    return FALSE;
  }
  provider->ready = TRUE;
  g_timeout_add_seconds(provider->hold_seconds, stop, NULL);
  return TRUE;
}

static gboolean set_clipboard_after_focus(gpointer user_data) {
  Provider *provider = user_data;
  if (provider->ready || provider->failed) return G_SOURCE_REMOVE;
  if (!establish_clipboard(provider, gdk_display_get_default())) gtk_main_quit();
  return G_SOURCE_REMOVE;
}

static gboolean schedule_clipboard_after_focus(GtkWidget *widget, GdkEventFocus *event, gpointer user_data) {
  (void)widget;
  (void)event;
  Provider *provider = user_data;
  if (!provider->ready && !provider->failed && !provider->selection_scheduled) {
    provider->selection_scheduled = TRUE;
    g_idle_add(set_clipboard_after_focus, provider);
  }
  return FALSE;
}

static gboolean write_ready(const gchar *path) {
  int descriptor = open(path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
  if (descriptor < 0) return FALSE;
  gboolean okay = write(descriptor, "ready\n", 6) == 6 && fsync(descriptor) == 0;
  close(descriptor);
  return okay;
}

int main(int argc, char **argv) {
  Provider provider = {.request_audit_descriptor = -1};
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
    } else if (index + 1 < argc && strcmp(argv[index], "--request-audit-file") == 0) {
      provider.request_audit_path = argv[index + 1];
      index += 2;
    } else if (strcmp(argv[index], "--require-focus") == 0) {
      provider.require_focus = TRUE;
      index++;
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
  provider.ready_path = ready_path;
  provider.hold_seconds = hold_seconds;
  gboolean needs_focus = provider.require_focus;
#ifdef GDK_WINDOWING_WAYLAND
  needs_focus = needs_focus || GDK_IS_WAYLAND_DISPLAY(gdk_display_get_default());
#endif
#ifdef GDK_WINDOWING_X11
  if (provider.request_audit_path != NULL && GDK_IS_X11_DISPLAY(gdk_display_get_default())) {
    if (!prepare_request_audit(&provider)) return 1;
    gdk_window_add_filter(NULL, audit_selection_request, &provider);
    provider.request_audit_filter_installed = TRUE;
  }
#endif
  GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title(GTK_WINDOW(window), "CopyPaste qualification source");
  gtk_window_set_default_size(GTK_WINDOW(window), 240, 80);
  if (needs_focus) {
    GtkWidget *focus_target = gtk_entry_new();
    gtk_container_add(GTK_CONTAINER(window), focus_target);
    g_signal_connect(focus_target, "focus-in-event", G_CALLBACK(schedule_clipboard_after_focus), &provider);
    gtk_widget_show_all(window);
    gtk_widget_grab_focus(focus_target);
  } else {
    gtk_widget_show_all(window);
    if (!establish_clipboard(&provider, gdk_display_get_default())) return 1;
  }
  gtk_main();
#ifdef GDK_WINDOWING_X11
  if (provider.request_audit_filter_installed) {
    gdk_window_remove_filter(NULL, audit_selection_request, &provider);
  }
#endif
  for (guint index = 0; index < provider.count; index++) {
    g_free(provider.offers[index].mime);
    g_free(provider.offers[index].bytes);
  }
  if (provider.request_audit_descriptor >= 0) close(provider.request_audit_descriptor);
  return provider.failed ? 1 : 0;
}
