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
    label: "Sync: {{state}}. {{detail}}",
    synced: "Synced",
    syncing: "Syncing",
    idle: "Idle",
    offline: "Offline",
    error: "Needs attention",
    detail: {
      serviceOffline: "Background service unreachable",
      serviceConnecting: "Connecting to the background service",
      serviceConnected: "Background service connected",
      protocolMismatch: "App and background service versions don't match",
      peersChecking: "Checking paired devices",
      peersUnavailable: "Peer sync unavailable",
      peersNone: "No paired devices",
      peers_one: "{{count}} paired device",
      peers_other: "{{count}} paired devices",
      peerLastSync: "Last peer sync {{age}}",
      peerNeverSynced: "No peer sync yet",
      stalled_one: "{{count}} peer not syncing",
      stalled_other: "{{count}} peers not syncing",
      cloudUnavailable: "Cloud sync unavailable in this build",
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
