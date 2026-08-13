export const history = {
  list: {
    label: "Clipboard history",
    loadMore: "Load more",
    loadingMore: "Loading…",
    unknownDevice: "Unknown device",
  },

  search: {
    label: "Search clipboard history",
    placeholder: "Search clipboard history",
    hint: "Search (⌘F) · ↓ to move into the list · ⌘A select all",
    clear: "Clear search",
    filterKind: "Filter by kind",
    filterDevice: "Filter by device",
    allDevices: "All devices",
    sortOrder: "Sort order",
    groupByDevice: "Group clipboard items by device",
    group: "Group",
    selectMultiple: "Select multiple items",
    leaveSelection: "Leave selection mode",
    clearAll: "Clear clipboard history",
    count_one: "{{count}} item",
    count_other: "{{count}} items",
    displayLimitHint:
      "Showing first {{limit}} of {{count}} results — adjust the display limit in Settings › Storage.",
  },

  kind: {
    all: "All kinds",
    text: "Text",
    image: "Images",
    file: "Files",
    url: "Links",
    mail: "Email addresses",
    path: "File paths",
    code: "Code",
    json: "JSON",
    num: "Numbers",
    color: "Colors",
    secret: "Sensitive",
    unknown: "Other",
  },

  sort: {
    newest: "Newest first",
    oldest: "Oldest first",
  },

  bulk: {
    label: "Selection actions",
    empty: "Select items to act on them",
    selected_one: "{{count}} item selected",
    selected_other: "{{count}} items selected",
    pin: "Pin",
    unpin: "Unpin",
    delete: "Delete",
    done: "Done",
  },

  /**
   * A11Y-3 is verbatim from manifest 06. `pinnedPrefix` and `selectPrefix` are
   * prefixes rather than `"Pinned. {{content}}"` templates: a clip's text is
   * concatenated onto them at the call site, so no catalogue entry can ever
   * carry item content.
   */
  row: {
    sensitiveName: "Sensitive item, hidden — activate to reveal",
    sensitiveReveal: "Sensitive content hidden — activate to reveal",
    sensitivePlaceholder: "Sensitive content hidden",
    empty: "Empty item",
    pinnedPrefix: "Pinned.",
    selectPrefix: "Select",
    selectingHint: "Click to add to the selection",
    hint: "Click to select · double-click to copy",
    tapHint: "Tap to copy",
    actions: "Item actions",
    hide: "Hide sensitive content",
    reveal: "Reveal sensitive content",
    copy: "Copy to clipboard",
    pin: "Pin item",
    unpin: "Unpin item",
    reorder: "Reorder pinned item",
    delete: "Delete item",
    pinnedBadge: "Pinned",
    sensitiveBadge: "· Sensitive",
    potentialSensitiveWarning: "Potentially sensitive content",
    potentialSensitiveBadge: "Potentially sensitive",
    showOriginal: "Show original content",
    hideOriginal: "Hide original content",
    open: "Show full contents",
    /** Concatenated with a device name at the call site, for the reason
     *  `pinnedPrefix` is a prefix. */
    fromPrefix: "From",
    wontSyncBadge: "Won't sync",
    wontSync: "Too large to sync — this item stays on this device",
    sourceUnavailable: "Source unavailable",
  },

  /**
   * The title carries no clip text and neither does anything else here: a
   * template that took an item's content as a variable would put the plaintext
   * somewhere the sensitive-content rules do not reach (`catalogue.test.ts`).
   */
  detail: {
    title: "Clipboard item",
    contents: "Item contents",
    image: "Image preview",
    imageLoading: "Loading image preview",
    imageUnavailable: "Image preview unavailable",
    empty: "This item has no text.",
    bodyUnavailable:
      "The rest of this item could not be read — what is shown is the shortened preview.",
    hide: "Hide again",
    copy: "Copy",
    copyImage: "Copy image",
  },

  empty: {
    loading: {
      title: "Loading…",
      body: "Fetching your clipboard history.",
    },
    starting: { title: "Starting up…" },
    failed: {
      title: "Failed to load history",
      support:
        "If this keeps happening, copy or export a diagnostic report for support.",
    },
    /**
     * The two states no retry can clear. Each says what happened, what is
     * still true, and the one thing that would actually change the outcome —
     * and `keyUnusable` says outright that there is nothing, because a screen
     * that pretends otherwise costs the user an evening.
     *
     * No path is named in any of them, and none may be: the location of the
     * database discloses the local username (AGENTS.md rule 4).
     */
    keyUnusable: {
      title: "This clipboard history can't be unlocked",
      body: "This device's encryption key is there but can't be used, so nothing can decrypt the history it protects — not this version, and not a later one. Trying again won't change that. Whatever had already synced to a paired device is still on that device.",
    },
    keyLocked: {
      title: "Waiting for the key store",
      body: "CopyPaste couldn't reach the key store that holds this device's encryption key, so your history stayed locked. Unlock it and try again.",
    },
    private: {
      title: "Private mode is on",
      body: "Clipboard items are not recorded while private mode is active.",
    },
    capturePaused: {
      title: "Clipboard capture is paused",
      body: "New clipboard items will appear here after capture is set up again.",
      action: "Set up capture",
    },
    /** `{{query}}` is the user's own search text, already visible in the field
     *  beside it. It is the one interpolation here that is not authored copy. */
    noResults: 'No results for "{{query}}"',
    noMatch: "Nothing matches this filter",
    filteredBody:
      "Try a different search term. Sensitive items are never indexed, so they never appear in results.",
    loadMore: "Load more history",
    none: {
      title: "Nothing copied yet",
      body: "Copy something and it will appear here.",
    },
  },

  skipped_one: "{{count}} item could not be read and is not shown.",
  skipped_other: "{{count}} items could not be read and are not shown.",

  reveal: {
    unavailable: "Sensitive content can't be shown here.",
    missing: "This item is no longer in your clipboard history.",
    failed: "CopyPaste couldn't show this sensitive item. Try again.",
    confirm: {
      title: "Reveal sensitive content?",
      body: "This item looks like a password, key or token. It will be shown for 10 seconds, and hidden again as soon as this window loses focus.",
      action: "Reveal",
    },
  },

  clear: {
    title: "Clear all clipboard history?",
    body: "This permanently deletes every unpinned item on this device. Pinned items are kept. This cannot be undone.",
    action: "Clear all",
  },

  bulkDelete: {
    title_one: "Delete {{count}} item?",
    title_other: "Delete {{count}} items?",
    body: "This permanently removes the selected clipboard items. Unlike deleting one item, this cannot be undone.",
    action: "Delete",
  },

  toast: {
    copied: "Copied — press ⌘V to paste",
    cleared_one: "Cleared {{count}} item — pinned items kept",
    cleared_other: "Cleared {{count}} items — pinned items kept",
    deleted: "Deleted",
    undo: "Undo",
    pinned_one: "Pinned {{count}} item",
    pinned_other: "Pinned {{count}} items",
    unpinned_one: "Unpinned {{count}} item",
    unpinned_other: "Unpinned {{count}} items",
    bulkDeleted_one: "Deleted {{count}} item",
    bulkDeleted_other: "Deleted {{count}} items",
    pinnedPartial: "Pinned {{done}} of {{total}} — {{failed}} failed",
    unpinnedPartial: "Unpinned {{done}} of {{total}} — {{failed}} failed",
    bulkDeletedPartial: "Deleted {{done}} of {{total}} — {{failed}} failed",
    bulkCopied_one: "Copied {{count}} item — press ⌘V to paste",
    bulkCopied_other: "Copied {{count}} items — press ⌘V to paste",
    // Named rather than counted: the skipped rows are protected content, and
    // "Copied 8 items" over a selection of ten reads as a success.
    bulkCopiedPartial:
      "Copied {{done}} of {{total}} — {{skipped}} left out as sensitive or an image",
    bulkCopyNothing: "Nothing was copied — every selected item is sensitive or an image",
  },
} as const;
