/* SPDX-License-Identifier: MIT */
#include "copypaste-clipboard-source.h"

#include <gmodule.h>
#include <meta/meta-selection.h>
#include <meta/meta-selection-source.h>

typedef gboolean (*WriterIdentityFunc) (MetaSelectionSource *source,
                                        guint32 *pid,
                                        guint32 *uid,
                                        gchar **status,
                                        gchar **app_id);
typedef MetaSelectionSource *(*CurrentOwnerFunc) (MetaSelection *selection,
                                                  MetaSelectionType selection_type);

static GModule *mutter_module;
static WriterIdentityFunc writer_identity;
static CurrentOwnerFunc current_owner;

static gboolean resolve_mutter_api (GError **error) {
  gpointer symbol = NULL;
  if (!mutter_module)
    mutter_module = g_module_open (NULL, G_MODULE_BIND_LAZY);
  if (!mutter_module || !g_module_symbol (mutter_module,
                                           "meta_selection_source_get_writer_identity",
                                           &symbol)) {
    g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_NOT_SUPPORTED,
                         "Mutter writer identity bridge is unavailable");
    return FALSE;
  }
  writer_identity = (WriterIdentityFunc) symbol;
  symbol = NULL;
  if (!g_module_symbol (mutter_module, "meta_selection_get_current_owner", &symbol)) {
    g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_NOT_SUPPORTED,
                         "Mutter current-owner bridge is unavailable");
    return FALSE;
  }
  current_owner = (CurrentOwnerFunc) symbol;
  return TRUE;
}

#define MAX_BYTES (4u * 1024u * 1024u)
#define MAX_TOTAL_BYTES (32u * 1024u * 1024u)
#define MAX_MIMES 64u
#define MAX_MIME_BYTES 255u

typedef struct {
  MetaSelectionSource parent;
  GHashTable *payloads;
  GPtrArray *mimes;
} CopyPasteClipboardSource;

static GType source_type;

static GList *source_get_mimetypes (MetaSelectionSource *source) {
  CopyPasteClipboardSource *self = (CopyPasteClipboardSource *) source;
  GList *result = NULL;
  for (guint i = 0; i < self->mimes->len; i++)
    result = g_list_append (result, g_strdup (g_ptr_array_index (self->mimes, i)));
  return result;
}

static void source_read_async (MetaSelectionSource *source, const gchar *mime,
                               GCancellable *cancellable, GAsyncReadyCallback callback,
                               gpointer user_data) {
  CopyPasteClipboardSource *self = (CopyPasteClipboardSource *) source;
  GTask *task = g_task_new (source, cancellable, callback, user_data);
  GBytes *bytes = g_hash_table_lookup (self->payloads, mime);
  if (!bytes) {
    g_task_return_new_error (task, G_IO_ERROR, G_IO_ERROR_NOT_FOUND, "Unsupported MIME type");
  } else {
    GInputStream *stream = g_memory_input_stream_new_from_bytes (bytes);
    g_task_return_pointer (task, stream, g_object_unref);
  }
  g_object_unref (task);
}

static GInputStream *source_read_finish (MetaSelectionSource *source,
                                         GAsyncResult *result, GError **error) {
  (void) source;
  return g_task_propagate_pointer (G_TASK (result), error);
}

static void source_finalize (GObject *object) {
  CopyPasteClipboardSource *self = (CopyPasteClipboardSource *) object;
  g_clear_pointer (&self->payloads, g_hash_table_unref);
  g_clear_pointer (&self->mimes, g_ptr_array_unref);
  G_OBJECT_CLASS (g_type_class_peek_parent (G_OBJECT_GET_CLASS (object)))->finalize (object);
}

static void source_class_init (gpointer klass, gpointer unused) {
  (void) unused;
  GObjectClass *object_class = G_OBJECT_CLASS (klass);
  MetaSelectionSourceClass *selection_class = (MetaSelectionSourceClass *) klass;
  object_class->finalize = source_finalize;
  selection_class->get_mimetypes = source_get_mimetypes;
  selection_class->read_async = source_read_async;
  selection_class->read_finish = source_read_finish;
}

static void source_init (GTypeInstance *instance, gpointer unused) {
  (void) unused;
  CopyPasteClipboardSource *self = (CopyPasteClipboardSource *) instance;
  self->payloads = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, (GDestroyNotify) g_bytes_unref);
  self->mimes = g_ptr_array_new_with_free_func (g_free);
}

static gboolean ensure_source_type (GError **error) {
  if (source_type)
    return TRUE;
  GType parent = g_type_from_name ("MetaSelectionSource");
  GTypeQuery query = {0};
  if (!parent) {
    g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_NOT_SUPPORTED, "MetaSelectionSource is unavailable");
    return FALSE;
  }
  g_type_query (parent, &query);
  if (query.instance_size != sizeof (MetaSelectionSource) || query.class_size != sizeof (MetaSelectionSourceClass)) {
    g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_NOT_SUPPORTED, "MetaSelectionSource ABI mismatch");
    return FALSE;
  }
  const GTypeInfo info = {
    .class_size = sizeof (MetaSelectionSourceClass), .class_init = source_class_init,
    .instance_size = sizeof (CopyPasteClipboardSource), .instance_init = source_init,
  };
  source_type = g_type_register_static (parent, "CopyPasteClipboardSource", &info, 0);
  return source_type != 0;
}

GObject *copypaste_clipboard_source_new (GVariant *payloads, GError **error) {
  if (!g_variant_is_of_type (payloads, G_VARIANT_TYPE ("a{say}")) || !ensure_source_type (error))
    return NULL;
  CopyPasteClipboardSource *self = (CopyPasteClipboardSource *) g_object_new (source_type, NULL);
  GVariantIter iter;
  gchar *mime;
  GVariant *value;
  guint64 total = 0;
  g_variant_iter_init (&iter, payloads);
  while (g_variant_iter_next (&iter, "{s@ay}", &mime, &value)) {
    gsize length = 0;
    const guint8 *data = g_variant_get_fixed_array (value, &length, sizeof (guint8));
    if (!*mime || strlen (mime) > MAX_MIME_BYTES || self->mimes->len >= MAX_MIMES || length > MAX_BYTES || total + length > MAX_TOTAL_BYTES || g_hash_table_contains (self->payloads, mime)) {
      g_free (mime); g_variant_unref (value); g_object_unref (self);
      g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_INVALID_ARGUMENT, "Invalid clipboard payloads");
      return NULL;
    }
    total += length;
    g_hash_table_insert (self->payloads, g_strdup (mime), g_bytes_new (data, length));
    g_ptr_array_add (self->mimes, mime);
    g_variant_unref (value);
  }
  if (!self->mimes->len) {
    g_object_unref (self);
    g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_INVALID_ARGUMENT, "Clipboard payloads are empty");
    return NULL;
  }
  return G_OBJECT (self);
}

GVariant *copypaste_clipboard_source_writer_identity (GObject *source,
                                                       GError **error) {
  guint32 uid = 0;
  guint32 pid = 0;
  gchar *status = NULL;
  gchar *app_id = NULL;
  GType selection_source_type = g_type_from_name ("MetaSelectionSource");
  if (!source || !selection_source_type ||
      !g_type_is_a (G_OBJECT_TYPE (source), selection_source_type)) {
    g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_INVALID_ARGUMENT,
                         "Not a Mutter selection source");
    return NULL;
  }
  if (!resolve_mutter_api (error) ||
      !writer_identity ((MetaSelectionSource *) source, &pid, &uid, &status, &app_id) || !status) {
    g_free (status);
    g_free (app_id);
    return NULL;
  }
  GVariant *identity = g_variant_ref_sink (g_variant_new ("(suus)", status, pid, uid,
                                                           app_id ? app_id : ""));
  g_free (status);
  g_free (app_id);
  return identity;
}

GObject *copypaste_clipboard_selection_owner (GObject *selection, GError **error) {
  if (!selection) {
    g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_INVALID_ARGUMENT,
                         "Missing Mutter selection");
    return NULL;
  }
  if (!resolve_mutter_api (error))
    return NULL;
  return G_OBJECT (current_owner ((MetaSelection *) selection, META_SELECTION_CLIPBOARD));
}
