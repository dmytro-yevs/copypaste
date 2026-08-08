import { settingsService } from "@/i18n/en/settingsService";

export const settings = {
  title: "Settings",
  sections: "Settings sections",

  search: {
    label: "Search settings",
    placeholder: "Search settings",
    clear: "Clear search",
    results: "Settings search results",
    opened: "Opened {{title}}.",
    empty: {
      title: "No settings found",
      body: "Try a broader term, such as “clipboard”, “sync” or “privacy”.",
    },
  },

  groups: {
    personal: "Personal",
    service: "CopyPaste",
    support: "Support",
  },

  tabs: {
    appearance: "Appearance",
    list: "List",
    shortcut: "Shortcut",
    service: "Service",
    sync: "Sync",
    storage: "Storage",
    diagnostics: "Diagnostics",
    about: "About",
  },

  appearance: {
    theme: {
      title: "Theme",
      system: "System",
      dark: "Dark",
      light: "Light",
      resolves: "Using {{theme}} now.",
      override: "Choose a theme just for CopyPaste.",
    },
    accent: {
      title: "Accent",
      description: "Colors buttons, selection and focus.",
      system: "System accent",
      indigo: "Indigo",
      blue: "Blue",
      teal: "Teal",
      green: "Green",
      amber: "Amber",
      rose: "Rose",
    },
    translucency: {
      title: "Translucency",
      description: "Controls how much of the window background shows through.",
    },
  },

  list: {
    previewLines: {
      title: "Preview lines",
      description: "Sets the preview height for each history item.",
      value_one: "{{count}} line",
      value_other: "{{count}} lines",
    },
    groupByDevice: {
      title: "Group by device",
      description: "Keeps items from the same device together.",
    },
    popupPreviewLines: {
      title: "Quick Paste preview lines",
      description: "Sets the preview height in Quick Paste.",
      value_one: "{{count}} line",
      value_other: "{{count}} lines",
    },
    historyDisplayLimit: {
      title: "History display limit",
      description: "Only limits what is shown; it never deletes history.",
      unlimited: "Unlimited",
    },
    warnBeforeReveal: {
      title: "Warn before revealing",
      description: "Ask before showing detected secrets. They hide after 10 seconds or when CopyPaste loses focus.",
    },
    allowScreenshots: {
      title: "Allow screenshots",
      description: "Lets CopyPaste appear in captures and app previews.",
      /** Shown only while the protection is off, because that is when it is a
       *  fact about the screen rather than a hypothetical. */
      warning:
        "Anything shown here can now be recorded, including a secret you reveal.",
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
    saved: "Saved",
    resetSaved: "Reset to default",
    saveFailed: "The previous shortcut is still in effect.",
    unavailable: "Shortcut settings are unavailable. {{accelerator}} remains active.",
    refusal: {
      mediaKeys:
        "Media keys can't be used: binding one needs macOS Accessibility permission, and CopyPaste loses that permission every time it updates. Pick another key.",
      /** Windows binds the other media keys with no permission at all, so only
       *  the two it has no key code for are refused — and for that reason,
       *  which is not the macOS one. */
      noKeyCode:
        "Windows has no key code for that media key, so it can't be bound. Pick another key.",
      needsModifier:
        "A shortcut needs at least one modifier — hold ⌘, ⌥ or ⌃ as well.",
      needsModifierWindows:
        "A shortcut needs at least one modifier — hold Ctrl, Alt or Shift as well.",
    },
  },

  startup: {
    title: "Startup",
    openAtLogin: {
      title: "Open CopyPaste at login",
      description: "Starts CopyPaste automatically when you sign in.",
    },
    /** The write can succeed while the machine still holds the app off, so the
     *  switch reports what it reads back rather than what was asked for. */
    blocked:
      "Your system is still set to skip CopyPaste at startup. Check the Startup apps list.",
    failed: "That setting couldn't be changed.",
  },

  sync: {
    paired: {
      title: "Paired devices",
      description: "Syncs directly between devices with end-to-end encryption.",
      none: "None",
      count: "{{n}} paired",
      manage: "Manage devices",
      /** Said of peer sync, which the service could not answer for. It is not
       *  the cloud row's badge, which is a standing fact about the build. */
      unavailable: "Unavailable",
    },
    now: {
      title: "Sync now",
      description: "Syncs every paired device now.",
      action: "Sync now",
      pending: "Syncing…",
    },
    cloud: {
      title: "Cloud sync",
      description: "Syncs encrypted history through your configured account.",
      notConfigured:
        "This build has no cloud deployment configured. Direct device sync still works.",
      badgeUnavailable: "Unavailable",
      badgeNotConfigured: "Not configured",
      badgeConnected: "Connected",
      badgeSignedOut: "Signed out",
      loading: "Checking account…",
      retry: "Try again",
      lastError: "The last cloud sync failed. Try again or sign in again.",
      formLabel: "Cloud account sign in",
      email: "Email",
      password: "Password",
      passphrase: "Sync passphrase",
      signIn: "Sign in",
      signingIn: "Signing in…",
      signOut: "Sign out",
      syncNow: "Sync cloud now",
      syncing: "Syncing…",
      toast: {
        signedIn: "Cloud sync connected",
        signedOut: "Signed out of cloud sync",
        synced: "Cloud sync finished: {{uploaded}} uploaded, {{downloaded}} downloaded",
        syncedWithSkips:
          "Cloud sync finished: {{uploaded}} uploaded, {{downloaded}} downloaded, {{skipped}} skipped",
      },
    },
  },

  service: settingsService,

  transfer: {
    export: {
      title: "Export history",
      description: "Saves readable text; anyone with the file can read it.",
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
      description: "Adds an export without replacing existing history.",
      action: "Import…",
      dialogTitle_one: "Import {{count}} item?",
      dialogTitle_other: "Import {{count}} items?",
      dialogBody_one:
        "This will import {{count}} item from the file into your clipboard history. Duplicate items will be skipped. Existing items are not deleted.",
      dialogBody_other:
        "This will import {{count}} items from the file into your clipboard history. Duplicate items will be skipped. Existing items are not deleted.",
      confirm: "Import",
      done_one: "Imported {{count}} item",
      done_other: "Imported {{count}} items",
      alreadyHere_one: "{{count}} was already here",
      alreadyHere_other: "{{count}} were already here",
    },

    backup: {
      title: "Back up history",
      description: "Saves an encrypted copy for this device without overwriting backups.",
      action: "Back up…",
      done: "Backed up — {{megabytes}} MB",
    },

    restore: {
      title: "Restore from a backup",
      description: "Replaces all local history. This cannot be undone.",
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
      description: "Saved on this device.",
    },
    clear: {
      title: "Clear history",
      description: "Deletes unpinned items here; pins and other devices are kept.",
      action: "Clear history",
    },
  },

  diagnostics: {
    loading: "Reading the service…",
    unavailable: "Diagnostics are unavailable.",
    views: "Diagnostics views",
    overview: "Overview",

    running: {
      title: "What is running",
      service: {
        title: "Clipboard service",
        running: "Running",
        stopped: "Stopped",
        notInstalled: "Not installed",
        unhealthy: "Answering, but unreadable",
        version: "CopyPaste {{version}}",
        mismatch: "A different version from this app.",
      },
      capture: {
        title: "Clipboard capture",
        running: "Running",
        stopped: "Stopped",
        description: "Stopped means new copies are not saved.",
      },
      backend: {
        title: "Clipboard backend",
        description: "Shows the clipboard source. A test source is not your real clipboard.",
      },
      history: {
        title: "History access",
        description: "Checks that saved items can be read.",
        readable: "Readable",
        failed: "Failed ({{code}})",
      },
      protocol: {
        title: "App and service agree",
        yes: "Yes",
        no: "No",
        description: "They must match for CopyPaste to work.",
      },
      started: {
        title: "Started",
        description: "Counts below start from this time.",
        unknown: "Not running",
      },
    },

    dropped: {
      title: "What has been dropped or refused",
      description: "Counts reset whenever the service restarts.",
      tooLarge: {
        title: "Copies refused for being too large",
        description: "They exceeded the capture size limit and were not saved.",
      },
      missed: {
        title: "Copies replaced before they could be read",
        description: "They changed before CopyPaste could read them.",
      },
      swept: {
        title: "Detected secrets deleted automatically",
        description: "Auto-deleted by your secret-retention setting. This cannot be undone.",
      },
      purged: {
        title: "Search entries removed at startup",
        description: "The items remain, but they are no longer searchable.",
      },
    },

    report: {
      title: "Report",
      description: "A support-ready summary of the current service state.",
      /** Rule 4, stated where the user decides whether to paste it: this is the
       *  promise the panel is making, so it goes beside the button. */
      safety: "It never includes copied content, paths or account names.",
      copy: "Copy diagnostics",
      copied: "Diagnostics copied",
      export: "Export support bundle",
      exporting: "Exporting support bundle…",
      exported: "Support bundle exported",
      empty: "Nothing to report yet.",
    },

    /** The sweep now announces itself. Before the service carried the count on
     *  its change event, an auto-deletion could only be noticed as an item that
     *  had gone missing. */
    swept_one: "{{count}} detected secret was deleted automatically",
    swept_other: "{{count}} detected secrets were deleted automatically",
  },

  about: {
    app: {
      title: "This app",
      description: "The app and service can have different versions.",
      version: "CopyPaste {{version}}",
    },
    service: {
      title: "Clipboard service",
      connecting: "Connecting…",
      version: "CopyPaste {{version}}",
    },
    capture: {
      title: "Clipboard capture",
      description: "Shows whether new copies are being saved.",
      running: "Running",
      paused: "Paused",
    },
    backend: {
      title: "Clipboard backend",
      description: "A test source is not your real clipboard.",
    },
    protocol: {
      title: "IPC protocol",
      description: "The app and service versions must match.",
      value: "v{{version}}",
      mismatch: "· app speaks v{{version}}",
    },
    items: {
      title: "Items stored",
      description: "Saved on this device.",
    },
    links: {
      title: "Links",
      repository: "Source code",
      releases: "Release notes",
    },
    reset: {
      title: "Reset preferences",
      description: "Restores appearance and list defaults. History stays intact.",
      action: "Reset preferences",
    },
  },
} as const;
