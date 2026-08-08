export const shell = {
  boundary: {
    navigation: "Navigation",
    title: "Something went wrong here",
    body: "{{region}} stopped working. The rest of CopyPaste is still running, and your clipboard history is untouched.",
    reload: "Reload this view",
  },

  /** One entry, because one condition reaches the banner. Offline, a protocol
   *  mismatch and a paused capture each have a surface that can also say what
   *  to do about it; this one has no recovery action anywhere. */
  banner: {
    keyUnusable:
      "This device's encryption key can't be used, so its clipboard history can't be unlocked. Nothing here can recover it.",
  },

  status: {
    label: "Sync: {{state}}. {{detail}}",
    openService: "Open service settings",
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
    },
  },

  service: {
    diagnostics: "Open diagnostics",
    outOfDate: {
      title: "The background service is out of date",
      body: "The clipboard service has a different version. Quit and reopen CopyPaste to update it.",
      restart: "Restart the service",
      restarting: "Restarting…",
    },
    notInstalled: {
      title: "This build has no background service",
      body: "CopyPaste records your clipboard from a small background service, and this build doesn't include one. Install CopyPaste from the official package to get it.",
    },
    stopped: {
      title: "The clipboard service isn't running",
      body: "CopyPaste saves new clipboard items through its background service. Start it to resume capture.",
      start: "Start the service",
      starting: "Starting…",
    },
    webPreview: {
      title: "This is the web preview",
      body: "Clipboard history is available in the native CopyPaste window, not in a browser tab.",
    },
  },
} as const;
