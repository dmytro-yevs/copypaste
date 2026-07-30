export const settings = {
  title: "Settings",
  sections: "Settings sections",

  tabs: {
    appearance: "Appearance",
    list: "List",
    shortcut: "Shortcut",
    service: "Service",
    sync: "Sync",
    storage: "Storage",
    about: "About",
  },

  appearance: {
    theme: {
      title: "Theme",
      system: "System",
      dark: "Dark",
      light: "Light",
      resolves: "Currently resolves to {{theme}}.",
      override: "Overrides the system appearance for CopyPaste only.",
    },
    accent: {
      title: "Accent",
      description: "Tints selection, focus rings and primary buttons.",
      indigo: "Indigo",
      blue: "Blue",
      teal: "Teal",
      green: "Green",
      amber: "Amber",
      rose: "Rose",
    },
    translucency: {
      title: "Translucency",
      description:
        "Frost the window chrome over what is behind it. Turned off automatically when the system asks for reduced transparency.",
    },
  },

  list: {
    previewLines: {
      title: "Preview lines",
      description:
        "How many lines of a clip each row shows. Every row reserves this much space whether it fills it or not, so the list never overlaps itself.",
      value_one: "{{count}} line",
      value_other: "{{count}} lines",
    },
    warnBeforeReveal: {
      title: "Warn before revealing",
      description:
        "Ask for confirmation before showing an item that looks like a password, key or token. Revealed items hide themselves again after 10 seconds, and whenever this window loses focus.",
    },
  },

  shortcut: {
    title: "Quick-paste shortcut",
    description: "Opens CopyPaste from anywhere. Hold at least one modifier.",
    /** A11Y-13: the accessible name spells the accelerator out, because a
     *  screen reader mangles "⌘⇧V" (CopyPaste-8ebg.53). */
    capturingName: "Press a key combination",
    capturingPrompt: "Press a key combination…",
    current:
      "Current shortcut: {{accelerator}}. Click and press a new key combination.",
    reset: "Reset to default",
    saveFailed: "The previous shortcut is still in effect.",
    unavailable:
      "This build can't change the shortcut yet — the background service owns it, and the app has no way to ask it to rebind. {{accelerator}} is in effect.",
    refusal: {
      mediaKeys:
        "Media keys can't be used: binding one needs macOS Accessibility permission, and CopyPaste loses that permission every time it updates. Pick another key.",
      needsModifier:
        "A shortcut needs at least one modifier — hold ⌘, ⌥ or ⌃ as well.",
    },
  },

  sync: {
    paired: {
      title: "Paired devices",
      description:
        "Sync is peer-to-peer and end-to-end encrypted: devices exchange history directly, and nothing passes through a server.",
      none: "None",
      count: "{{n}} paired",
      manage: "Manage devices",
    },
    now: {
      title: "Sync now",
      description:
        "Runs a sync with every paired device. Devices also sync on their own when they see each other.",
      action: "Sync now",
      pending: "Syncing…",
    },
    cloud: {
      title: "Cloud sync",
      description:
        "Not in this build. The service has no cloud-account command for the app to call, so signing in would be a form with nothing behind it.",
      badge: "Unavailable",
    },
    unavailable:
      "Device sync isn't available in this build — pairing runs in the background service, which this build doesn't reach.",
  },

  service: {
    unavailable:
      "This build can't change the background service's settings — it has no service to change them on, and runs on the defaults shown here.",
    loading: "Reading the service's settings…",

    /** Shown at the field, not in a footnote: `lan_visibility` is the one
     *  setting `ConfigData::field_liveness` marks as read once at start. */
    liveness: {
      needsRestart: "Needs a restart",
      needsRestartWhy:
        "The service announces itself when it starts, so this takes effect the next time it does. It is saved either way.",
      pending: "Saved. It takes effect when the background service restarts.",
      restart: "Restart the service",
      restarting: "Restarting…",
      restarted: "The background service was restarted",
    },

    poll: {
      title: "Check the clipboard every",
      description:
        "How often the service looks for something new. Checking less often uses less power and can miss a copy you immediately replace.",
    },
    historyLimit: {
      title: "Keep at most",
      description:
        "When history passes this, the oldest unpinned items are dropped. Pinned items are never dropped.",
    },
    retention: {
      title: "Drop items older than",
      description:
        "Items past this age are deleted whether or not history is full. Pinned items are kept.",
    },
    dedup: {
      title: "Treat a repeat as the same item for",
      description:
        "Copying the same thing twice inside this window keeps one entry and moves it back to the top.",
    },
    maxItem: {
      title: "Ignore copies larger than",
      description:
        "Anything bigger is not captured at all. Raising this stores more; lowering it skips the large pastes that a clipboard manager is least useful for.",
    },

    sensitive: {
      title: "Delete detected secrets after",
      description:
        "Items the detector reads as a password, key or token are deleted this long after they are copied. Off by default.",
      /** Rule 4: auto-deletion is unrecoverable, so turning it on says so at
       *  the control rather than in a manual. */
      warning:
        "While this is on, anything the detector flags is deleted without asking, and a wrongly flagged item goes with it. Nothing that is deleted can be brought back.",
      /** The honest half: the service reports no event when the sweep runs, so
       *  the app cannot announce a deletion as it happens. */
      unannounced:
        "The service doesn't report when this runs, so a deletion shows up only as an item that is no longer in the list.",
      off: "Off — flagged items are kept until you delete them.",
    },

    notify: {
      title: "Notify on capture",
      description:
        "Post a notification when something is captured while CopyPaste is in the background.",
    },
    sound: {
      title: "Sound on capture",
      description: "Play a short sound when something is captured.",
    },
    lan: {
      title: "Visible on this network",
      description:
        "Lets paired devices find this one by name instead of by address. Turning it off does not unpair anything, and does not stop syncing with a device that already knows where this one is.",
    },
    syncEnabled: {
      title: "Sync with paired devices",
      description:
        "The master switch. With this off, nothing is sent to or accepted from any device, and pairings are kept.",
    },

    units: {
      off: "Off",
      ms: "{{count}} ms",
      seconds_one: "{{count}} second",
      seconds_other: "{{count}} seconds",
      minutes_one: "{{count}} minute",
      minutes_other: "{{count}} minutes",
      hours_one: "{{count}} hour",
      hours_other: "{{count}} hours",
      days_one: "{{count}} day",
      days_other: "{{count}} days",
      items_one: "{{count}} item",
      items_other: "{{count}} items",
      kilobytes: "{{count}} KB",
      megabytes: "{{count}} MB",
      never: "Never",
      /** A value the service accepts that this screen does not offer — set
       *  from the CLI. Shown as itself rather than snapped to a neighbour. */
      raw: "{{count}}",
    },
  },

  transfer: {
    unavailable:
      "This build can't move history in or out — that runs in the background service, which this build doesn't reach.",

    export: {
      title: "Export history",
      description:
        "Writes your clips out as readable text you can keep or move elsewhere. Anything that can read what you write is able to read your history.",
      action: "Export…",
      dialogTitle: "Export your clipboard history?",
      dialogBody:
        "What is written is plain readable text, not encrypted. Keep it somewhere you would keep a document full of passwords.",
      includeSensitive: "Include items that look like passwords or keys",
      includeSensitiveHint:
        "Off by default. Turning it on writes detected credentials out in the clear.",
      confirm: "Choose where to save",
      done_one: "Exported {{count}} item",
      done_other: "Exported {{count}} items",
      withheld_one: "{{count}} that looks like a secret was left out",
      withheld_other: "{{count}} that look like secrets were left out",
      unreadable_one: "{{count}} could not be read",
      unreadable_other: "{{count}} could not be read",
      nonText_one: "{{count}} was not text",
      nonText_other: "{{count}} were not text",
    },

    import: {
      title: "Import history",
      description:
        "Reads an export back in. Items already here are kept — an import adds, it never replaces.",
      action: "Import…",
      dialogTitle: "Add these clips to this device?",
      dialogBody:
        "Every clip is added to the history you already have. Nothing here is deleted. Each one is checked again as it goes in, so anything that looks like a secret is stored as one and repeats collapse into what is already here.",
      confirm: "Choose what to import",
      done_one: "Imported {{count}} item",
      done_other: "Imported {{count}} items",
      alreadyHere_one: "{{count}} was already here",
      alreadyHere_other: "{{count}} were already here",
    },

    backup: {
      title: "Back up history",
      description:
        "Writes an encrypted copy of everything, readable only by this device. A backup is never written over something that already exists, so give it a new name.",
      action: "Back up…",
      done: "Backed up — {{megabytes}} MB",
    },

    restore: {
      title: "Restore from a backup",
      description: "Replaces everything on this device with a backup's contents.",
      action: "Restore…",
      dialogTitle: "Replace this device's history?",
      dialogBody:
        "Every clip on this device is deleted and replaced with the backup's — pinned items included. What is here now is not saved anywhere first, and this cannot be undone.",
      dialogSafety:
        "The backup is opened with this device's own key and checked before anything is replaced, so one that is damaged or came from another device leaves your history exactly as it is.",
      confirm: "Choose a backup",
      done: "History restored from the backup",
    },
  },

  storage: {
    stored: {
      title: "Items stored",
      description: "Held by the background service for this device.",
    },
    clear: {
      title: "Clear history",
      description:
        "Deletes every unpinned item on this device. Pinned items are kept, and paired devices keep their own copies.",
      action: "Clear history",
    },
  },

  about: {
    service: {
      title: "Background service",
      connecting: "Connecting…",
      version: "CopyPaste {{version}}",
    },
    capture: {
      title: "Clipboard capture",
      description:
        "Whether the service is watching the system clipboard right now.",
      running: "Running",
      paused: "Paused",
    },
    backend: {
      title: "Clipboard backend",
      description:
        "Which clipboard the service is reading. A test backend means this build is not touching your real clipboard.",
    },
    protocol: {
      title: "IPC protocol",
      description: "The app and the service have to agree on this.",
      value: "v{{version}}",
      mismatch: "· app speaks v{{version}}",
    },
    items: {
      title: "Items stored",
      description: "Everything the service is holding for this device.",
    },
    reset: {
      title: "Reset preferences",
      description:
        "Puts theme, accent and list settings back to their defaults. Your clipboard history is not touched.",
      action: "Reset preferences",
    },
  },
} as const;
