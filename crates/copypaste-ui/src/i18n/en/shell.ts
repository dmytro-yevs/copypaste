export const shell = {
  boundary: {
    navigation: "Navigation",
    title: "{{region}} didn’t open",
    body: "This view is temporarily unavailable, while other CopyPaste screens remain available.",
    reload: "Try again",
    diagnostics: "Open diagnostics",
  },

  loading: "Loading {{region}}…",

  /** One entry, because one condition reaches the banner. Offline, a protocol
   *  mismatch and a paused capture each have a surface that can also say what
   *  to do about it; this one has no recovery action anywhere. */
  banner: {
    keyUnusable:
      "This device's encryption key can't be used, so its clipboard history can't be unlocked. Nothing here can recover it.",
  },

  service: {
    diagnostics: "Open diagnostics",
    checking: {
      title: "Checking the clipboard service…",
      body: "CopyPaste is checking whether the background service is available.",
    },
    running: {
      title: "The clipboard service is running",
      refreshing: "CopyPaste is refreshing your clipboard history.",
      retry: "The service is available, but clipboard history still hasn't loaded. Try again or open diagnostics.",
    },
    unhealthy: {
      title: "The clipboard service isn't responding correctly",
      body: "CopyPaste found the service, but couldn't read its status. Try again or open diagnostics.",
    },
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
