/* Private KWin geometry cache for the disposable Linux qualification profile. */
#include <gio/gio.h>
#include <limits.h>
#include <string.h>

#define BUS_NAME "app.copypaste.QualificationGeometry"
#define PATH "/app/copypaste/QualificationGeometry"
#define INTERFACE "app.copypaste.QualificationGeometry"
#define MAX_RECORDS 128

typedef struct { guint pid; gchar *title; gchar *id; gint values[8]; gint stacking; } Record;
static GHashTable *records;
static GDBusNodeInfo *node;
static guint compositor_pid;

static const gchar xml[] =
  "<node><interface name='" INTERFACE "'>"
  "<method name='Version'><arg type='u' direction='out'/></method>"
  "<method name='Report'><arg type='i' direction='in'/><arg type='s' direction='in'/><arg type='s' direction='in'/><arg type='s' direction='in'/><arg type='s' direction='in'/></method>"
  "<method name='Remove'><arg type='s' direction='in'/></method>"
  "<method name='Lookup'><arg type='u' direction='in'/><arg type='s' direction='in'/><arg type='i' direction='out'/><arg type='i' direction='out'/><arg type='i' direction='out'/><arg type='i' direction='out'/><arg type='i' direction='out'/><arg type='i' direction='out'/><arg type='i' direction='out'/><arg type='i' direction='out'/><arg type='i' direction='out'/></method>"
  "<method name='PointBelongsTo'><arg type='u' direction='in'/><arg type='s' direction='in'/><arg type='i' direction='in'/><arg type='i' direction='in'/><arg type='b' direction='out'/></method>"
  "</interface></node>";

static void record_free(Record *record) { if (record) { g_free(record->title); g_free(record->id); g_free(record); } }
static gboolean valid_text(const gchar *value, gsize maximum) { return value && *value && strlen(value) <= maximum && g_utf8_validate(value, -1, NULL); }
static gboolean parse_rect(const gchar *text, gint output[4], gint *stacking) {
  gchar *end = NULL;
  for (guint index = 0; index < 5; index++) {
    gint64 value = g_ascii_strtoll(index == 0 ? text : end + 1, &end, 10);
    if (end == NULL || (index < 4 && *end != ',') || value < -32768 || value > 32768) return FALSE;
    if (index < 4) output[index] = (gint)value; else *stacking = (gint)value;
  }
  return *end == '\0' && output[2] > 0 && output[3] > 0;
}
static void reply_error(GDBusMethodInvocation *invocation, const gchar *code) { g_dbus_method_invocation_return_dbus_error(invocation, INTERFACE ".Error.Invalid", code); }
static gboolean is_compositor_sender(GDBusConnection *connection, const gchar *sender) {
  GError *error = NULL;
  GVariant *reply = g_dbus_connection_call_sync(connection, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixProcessID", g_variant_new("(s)", sender), G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, 1000, NULL, &error);
  if (!reply) { g_clear_error(&error); return FALSE; }
  guint pid = 0; g_variant_get(reply, "(u)", &pid); g_variant_unref(reply); return pid == compositor_pid;
}
static void method_call(GDBusConnection *connection, const gchar *sender, const gchar *object_path, const gchar *interface_name, const gchar *method, GVariant *parameters, GDBusMethodInvocation *invocation, gpointer data) {
  (void)object_path; (void)interface_name; (void)data;
  if (g_str_equal(method, "Version")) { g_dbus_method_invocation_return_value(invocation, g_variant_new("(u)", 1)); return; }
  if (g_str_equal(method, "Report")) {
    gint pid; const gchar *title; const gchar *id; const gchar *frame; const gchar *buffer;
    if (!is_compositor_sender(connection, sender)) { reply_error(invocation, "untrusted compositor sender"); return; }
    g_variant_get(parameters, "(i&s&s&s&s)", &pid, &title, &id, &frame, &buffer);
    gint values[8], frame_stacking = 0, buffer_stacking = 0;
    if (pid < 2 || !valid_text(title, 256) || !valid_text(id, 128) || !parse_rect(frame, values, &frame_stacking) || !parse_rect(buffer, values + 4, &buffer_stacking) || frame_stacking != buffer_stacking) { reply_error(invocation, "invalid report"); return; }
    Record *record = g_hash_table_lookup(records, id);
    if (!record) { if (g_hash_table_size(records) >= MAX_RECORDS) { reply_error(invocation, "too many windows"); return; } record = g_new0(Record, 1); record->id = g_strdup(id); g_hash_table_insert(records, record->id, record); }
    record->pid = (guint)pid; record->stacking = frame_stacking; g_free(record->title); record->title = g_strdup(title); memcpy(record->values, values, sizeof(values));
    g_dbus_method_invocation_return_value(invocation, NULL); return;
  }
  if (g_str_equal(method, "Remove")) { const gchar *id; if (!is_compositor_sender(connection, sender)) { reply_error(invocation, "untrusted compositor sender"); return; } g_variant_get(parameters, "(&s)", &id); if (!valid_text(id, 128)) { reply_error(invocation, "invalid id"); return; } g_hash_table_remove(records, id); g_dbus_method_invocation_return_value(invocation, NULL); return; }
  if (g_str_equal(method, "Lookup")) {
    guint pid; const gchar *title; Record *match = NULL;
    g_variant_get(parameters, "(u&s)", &pid, &title);
    if (pid < 2 || !valid_text(title, 256)) { reply_error(invocation, "invalid lookup"); return; }
    GHashTableIter iter; gpointer value; g_hash_table_iter_init(&iter, records);
    while (g_hash_table_iter_next(&iter, NULL, &value)) { Record *record = value; if (record->pid == pid && g_str_equal(record->title, title)) { if (match) { reply_error(invocation, "ambiguous window"); return; } match = record; } }
    if (!match) { reply_error(invocation, "window unavailable"); return; }
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(iiiiiiiii)", match->values[0], match->values[1], match->values[2], match->values[3], match->values[4], match->values[5], match->values[6], match->values[7], match->stacking)); return;
  }
  if (g_str_equal(method, "PointBelongsTo")) {
    guint pid; gint x, y; const gchar *title; Record *match = NULL;
    g_variant_get(parameters, "(u&sii)", &pid, &title, &x, &y);
    if (pid < 2 || !valid_text(title, 256)) { reply_error(invocation, "invalid point lookup"); return; }
    GHashTableIter iter; gpointer value; g_hash_table_iter_init(&iter, records);
    while (g_hash_table_iter_next(&iter, NULL, &value)) { Record *record = value; if (record->pid == pid && g_str_equal(record->title, title)) { if (match) { reply_error(invocation, "ambiguous window"); return; } match = record; } }
    gboolean owned = match && x >= match->values[4] && y >= match->values[5] && x < match->values[4] + match->values[6] && y < match->values[5] + match->values[7];
    if (owned) { g_hash_table_iter_init(&iter, records); while (g_hash_table_iter_next(&iter, NULL, &value)) { Record *record = value; if (record != match && record->stacking > match->stacking && x >= record->values[4] && y >= record->values[5] && x < record->values[4] + record->values[6] && y < record->values[5] + record->values[7]) { owned = FALSE; break; } } }
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", owned)); return;
  }
  reply_error(invocation, "unknown method");
}
static const GDBusInterfaceVTable vtable = { method_call, NULL, NULL, {0} };
static void bus_acquired(GDBusConnection *connection, const gchar *name, gpointer loop) {
  (void)name; GError *error = NULL;
  if (!g_dbus_connection_register_object(connection, PATH, node->interfaces[0], &vtable, NULL, NULL, &error)) { g_printerr("%s\n", error->message); g_error_free(error); g_main_loop_quit(loop); }
}
static void name_lost(GDBusConnection *connection, const gchar *name, gpointer loop) { (void)connection; (void)name; g_main_loop_quit(loop); }
int main(int argc, char **argv) {
  if (argc != 3 || g_strcmp0(argv[1], "--compositor-pid") != 0) return 2;
  gchar *end = NULL; guint64 parsed = g_ascii_strtoull(argv[2], &end, 10);
  if (!end || *end || parsed < 2 || parsed > G_MAXUINT) return 2;
  compositor_pid = (guint)parsed;
  GError *error = NULL; node = g_dbus_node_info_new_for_xml(xml, &error); if (!node) { g_printerr("%s\n", error->message); g_error_free(error); return 1; }
  records = g_hash_table_new_full(g_str_hash, g_str_equal, NULL, (GDestroyNotify)record_free);
  GMainLoop *loop = g_main_loop_new(NULL, FALSE); guint owner = g_bus_own_name(G_BUS_TYPE_SESSION, BUS_NAME, G_BUS_NAME_OWNER_FLAGS_NONE, bus_acquired, NULL, name_lost, loop, NULL);
  g_main_loop_run(loop); g_bus_unown_name(owner); g_main_loop_unref(loop); g_hash_table_unref(records); g_dbus_node_info_unref(node); return 0;
}
