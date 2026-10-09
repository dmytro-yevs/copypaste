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

G_END_DECLS
