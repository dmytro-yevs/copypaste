export const shell = {
  boundary: {
    navigation: "Navigation",
    title: "Something went wrong here",
    body: "{{region}} stopped working. The rest of CopyPaste is still running, and your clipboard history is untouched.",
    reload: "Reload this view",
  },

  banner: {
    serviceOffline:
      "Background service not running — nothing is being recorded.",
    legacyDatabase:
      "This device's clipboard history was written by CopyPaste 0.4 — this version can't read it, and hasn't changed it.",
    keyUnusable:
      "This device's encryption key can't be used, so its clipboard history can't be unlocked. Nothing here can recover it.",
    protocolMismatch:
      "CopyPaste and the background service are on incompatible versions (service protocol v{{protocol}}). Restart both to resolve.",
    capturePaused:
      "The clipboard service is running but is not recording — copied items will not appear here.",
    /** Not "can't read it" alone: the news is that nothing was thrown away.
     *  An empty list after an upgrade reads as data loss otherwise. */
    legacyHistory:
      "Your CopyPaste 0.4 history is still on this device, unchanged. This version can't read it, so it has started a new one.",
  },

  status: {
    label: "Clipboard service: {{state}}. {{detail}}",
    running: "Service running",
    paused: "Capture paused",
    starting: "Starting up",
    offline: "Service offline",
    error: "Service error",
    detail: {
      offline: "The background service is not running.",
      starting: "The clipboard service is initializing.",
      connecting: "Connecting to the background service.",
      paused: "The service is running but is not recording the clipboard.",
      mismatch: "CopyPaste {{version}}, on a different protocol version.",
      running_one: "CopyPaste {{version}} · {{count}} item",
      running_other: "CopyPaste {{version}} · {{count}} items",
    },
  },

  service: {
    recheck: "Check again",
    outOfDate: {
      title: "The background service is out of date",
      ours: "A different version of the background service is running. Restart it to pick up this update.",
      theirs:
        "A different version of the background service is running, and it was started by something else — CopyPaste can't stop it. Quit it, then restart.",
      restart: "Restart the service",
      restarting: "Restarting…",
    },
    notInstalled: {
      title: "This build has no background service",
      body: "CopyPaste records your clipboard from a small background service, and this build doesn't include one. Install CopyPaste from the official package to get it.",
    },
    stopped: {
      title: "The clipboard service isn't running",
      body: "CopyPaste records your clipboard from a small background service. It isn't running, so nothing is being saved right now.",
      start: "Start the service",
      starting: "Starting…",
    },
  },
} as const;
