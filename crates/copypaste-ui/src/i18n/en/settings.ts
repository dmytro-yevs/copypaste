export const settings = {
  title: "Settings",
  sections: "Settings sections",

  tabs: {
    appearance: "Appearance",
    list: "List",
    shortcut: "Shortcut",
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

  storage: {
    stored: {
      title: "Items stored",
      description: "Held by the background service for this device.",
    },
    retention: {
      title: "How long items are kept",
      description:
        "The service enforces its own age and count limits. It exposes no command to read or change them, so this screen would be guessing if it showed a number.",
      badge: "Set by the service",
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
