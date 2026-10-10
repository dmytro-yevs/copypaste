/* SPDX-License-Identifier: MIT */
#pragma once

#include <gio/gio.h>

G_BEGIN_DECLS

/**
 * copypaste_clipboard_source_new:
 * @payloads: (not nullable): an `a{say}` bounded clipboard payload map
 * @error: (out) (optional): return location for a #GError
 *
 * Returns: (transfer full) (nullable): a new bounded selection source.
 */
GObject *copypaste_clipboard_source_new (GVariant *payloads, GError **error);

/** Returns a `(suus)` writer identity tuple, or NULL when unavailable. */
GVariant *copypaste_clipboard_source_writer_identity (GObject *source,
                                                       GError **error);

/**
 * Returns: (transfer none) (nullable): current clipboard owner, borrowed from
 * Mutter and valid only while @selection remains alive.
 */
GObject *copypaste_clipboard_selection_owner (GObject *selection,
                                               GError **error);

G_END_DECLS
