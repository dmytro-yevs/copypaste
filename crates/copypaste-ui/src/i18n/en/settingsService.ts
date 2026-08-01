export const settingsService = {
  unavailable: "Service settings are unavailable.",
  loading: "Reading the service's settings…",

  /** Eleven settings covering four unrelated things read as one long list, and
   *  a user looking for any one of them read all nine. */
  groups: {
    capture: {
      title: "What gets captured",
      description: "Anything refused here is not saved.",
    },
    keeping: {
      title: "What gets kept",
    },
    telling: {
      title: "What you're told",
    },
    network: {
      title: "Other devices",
    },
  },

  /** Shown at the field, not in a footnote: `lan_visibility` is the one
   *  setting `ConfigData::field_liveness` marks as read once at start. */
  liveness: {
    needsRestart: "Needs a restart",
    needsRestartWhy:
      "This takes effect after the service restarts.",
    pending: "Saved. It takes effect after the service restarts.",
    restart: "Restart the service",
    restarting: "Restarting…",
    restarted: "The background service was restarted",
  },

  poll: {
    title: "Check the clipboard every",
    description: "Shorter intervals use more power and catch faster copies.",
  },
  privateMode: {
    title: "Private mode",
    description: "Pauses recording until you turn it off.",
  },
  exclusions: {
    title: "Exclude apps from capture",
    description: "Matching apps are never read or saved. Unknown apps are skipped while this list is not empty.",
    androidLimitation: "An exclusion applies when the active Android capture method reports the source app. CopyPaste does not request Usage Access or Accessibility for this.",
    inputLabel: "App bundle or package ID",
    placeholder: "com.example.app",
    searchInstalled: "Search installed apps",
    appsUnavailable: "Installed apps couldn't be read.",
    noInstalledMatches: "No installed apps match.",
    add: "Add app",
    remove: "Remove {{id}}",
    seen: "Apps seen in this history",
    noMatches: "No matching app was seen.",
    invalid: "Enter a bundle or package ID, such as com.example.app.",
    exists: "That app is already excluded.",
  },
  historyLimit: {
    title: "Keep at most",
    description: "Drops the oldest unpinned items when the limit is reached.",
  },
  storageQuota: {
    title: "Storage quota",
    description: "Drops the oldest unpinned items when this size is reached.",
  },
  retention: {
    title: "Drop items older than",
    description: "Deletes unpinned items older than this age.",
  },
  dedup: {
    title: "Treat a repeat as the same item for",
    description: "Repeated copies update one item instead of adding another.",
  },
  maxText: {
    title: "Ignore text larger than",
    description: "Larger text is not captured.",
  },
  maxImage: {
    title: "Ignore images larger than",
    description: "Larger images are not captured.",
  },
  maxFile: {
    title: "Ignore files larger than",
    description: "Larger files are not captured; effective limit is 4 MiB.",
  },
  maxDecodedImage: {
    title: "Decoded image memory limit",
    description: "Prevents large images from exhausting memory.",
  },
  validation: {
    poll: "Choose an interval from 100 ms through 5000 ms.",
    text: "Choose a text limit of at least 64 KB.",
    image: "Choose an image limit of at least 1 MB.",
    file: "Choose a file limit from 1 MB through 100 MB.",
    decodedImage: "Choose a decoded image budget of at least 1 MB.",
  },

  sensitive: {
    title: "Delete detected secrets after",
    description: "Deletes detected passwords, keys and tokens after this time.",
    /** Rule 4: auto-deletion is unrecoverable, so turning it on says so at
     *  the control rather than in a manual. */
    warning:
      "Detected secrets are deleted without asking and cannot be recovered.",
    /** Was "the service doesn't report when this runs". It does now — the
     *  change event carries the count — so the warning names where the
     *  running total is instead of admitting there isn't one. */
    announced:
      "Each deletion is announced and counted in Diagnostics.",
    off: "Off — flagged items are kept until you delete them.",
  },

  notify: {
    title: "Notify on capture",
    description: "Only when CopyPaste is in the background.",
  },
  sound: {
    title: "Sound on capture",
    description: "Plays when a new item is captured.",
  },
  lan: {
    title: "Visible on this network",
    description: "Lets paired devices find this one on the local network.",
  },
  syncEnabled: {
    title: "Sync with paired devices",
    description: "Stops syncing without removing pairings.",
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
    gigabytes: "{{count}} GB",
    never: "Never",
    /** A value the service accepts that this screen does not offer — set
     *  from the CLI. Shown as itself rather than snapped to a neighbour. */
    raw: "{{count}}",
  },
} as const;
