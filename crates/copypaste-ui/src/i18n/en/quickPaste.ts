/**
 * The popup surface's strings.
 *
 * `copyPrefix` is a prefix rather than a `"Copy {{content}}"` template, for the
 * same reason as `history.row.selectPrefix`: a clipping must never be an
 * argument to `t`.
 */
export const quickPaste = {
  title: "Quick Paste",

  search: {
    label: "Search clipboard history",
  },

  loading: {
    title: "Loading clipboard history",
    body: "Looking for your recent copies.",
  },
  offline: {
    title: "The clipboard service isn't running",
    body: "Start it to see and copy recent items.",
    action: "Restart service",
  },
  starting: {
    title: "Starting clipboard service",
    body: "Your recent copies will appear as soon as it is ready.",
  },
  failed: {
    title: "Couldn't load clipboard history",
    body: "Try again. If this keeps happening, open Settings to view diagnostics.",
  },
  /** `{{query}}` is the user's own search text, already visible in the field
   *  beside this message. */
  noResults: {
    title: "No matches for “{{query}}”",
    body: "Try a different word or clear the search.",
  },
  empty: {
    title: "Nothing copied yet",
    body: "Copies from your device will appear here.",
  },

  settings: "Open Settings",

  count: {
    loading: "Loading…",
    /** Two numbers rather than one: a filtered list that says only its own
     *  length reads as the whole history having shrunk. */
    partial: "{{shown}} of {{total}}",
    all_one: "{{count}} item",
    all_other: "{{count}} items",
  },

  row: {
    sensitive: "Sensitive content",
    image: "Image",
    file: "File",
    empty: "Empty item",
    pinned: "Pinned",
    pin: "Pin",
    unpin: "Unpin",
    copyPrefix: "Copy",
  },

  toast: {
    restartFailed: "Couldn’t restart the clipboard service.",
    copyFailed: "Couldn’t copy that item.",
    pinFailed: "Couldn’t pin that item.",
    unpinFailed: "Couldn’t unpin that item.",
    settingsFailed: "Couldn’t open Settings.",
    retry: "Retry",
  },
} as const;
