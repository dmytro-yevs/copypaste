export const settingsService = {
  unavailable:
    "This build can't change the background service's settings — it has no service to change them on, and runs on the defaults shown here.",
  loading: "Reading the service's settings…",

  /** Eleven settings covering four unrelated things read as one long list, and
   *  a user looking for any one of them read all nine. */
  groups: {
    capture: {
      title: "What gets captured",
      description:
        "Anything refused here is never saved, and the counts are on the Diagnostics tab.",
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
      "The service announces itself when it starts, so this takes effect the next time it does. It is saved either way.",
    pending: "Saved. It takes effect when the background service restarts.",
    restart: "Restart the service",
    restarting: "Restarting…",
    restarted: "The background service was restarted",
  },

  poll: {
    title: "Check the clipboard every",
    description:
      "How often the service looks for something new. Checking less often uses less power; anything you copy and replace faster than this is missed, and counted on the Diagnostics tab.",
  },
  privateMode: {
    title: "Private mode",
    description:
      "Pause clipboard recording until you turn it off. This stays on after CopyPaste restarts.",
  },
  historyLimit: {
    title: "Keep at most",
    description:
      "When history passes this, the oldest unpinned items are dropped. Pinned items are never dropped.",
  },
  storageQuota: {
    title: "Storage quota",
    description:
      "When unpinned clips exceed this size, the oldest are dropped. Pinned clips and the newest unpinned clip are kept.",
  },
  retention: {
    title: "Drop items older than",
    description:
      "Items past this age are deleted whether or not history is full. Pinned items are kept. Never means age alone never deletes anything — the limit above still does.",
  },
  dedup: {
    title: "Treat a repeat as the same item for",
    description:
      "Copying the same thing twice inside this window keeps one entry and moves it back to the top. Off keeps every copy as its own entry, so copying one thing repeatedly fills the history with it.",
  },
  maxText: {
    title: "Ignore text larger than",
    description:
      "Text above this configured size is not captured. Minimum: 64 KB; configured default: 10 MB. This build's storage and transport safety ceiling makes the effective limit at most 4 MiB.",
  },
  maxImage: {
    title: "Ignore images larger than",
    description:
      "Encoded image data above this configured size is not captured. Minimum: 1 MB; configured default: 64 MB. This build's storage and transport safety ceiling makes the effective limit at most 4 MiB.",
  },
  maxFile: {
    title: "Ignore files larger than",
    description:
      "Files above this configured size are not captured. The configured hard maximum and default are 100 MB. This build's storage and transport safety ceiling makes the effective limit at most 4 MiB.",
  },
  maxDecodedImage: {
    title: "Decoded image memory limit",
    description:
      "Compressed images can expand sharply in memory. Images whose decoded bitmap exceeds this live safety budget are refused. Default: 50 MB.",
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
    description:
      "Items the detector reads as a password, key or token are deleted this long after they are copied. Off by default.",
    /** Rule 4: auto-deletion is unrecoverable, so turning it on says so at
     *  the control rather than in a manual. */
    warning:
      "While this is on, anything the detector flags is deleted without asking, and a wrongly flagged item goes with it. Nothing that is deleted can be brought back.",
    /** Was "the service doesn't report when this runs". It does now — the
     *  change event carries the count — so the warning names where the
     *  running total is instead of admitting there isn't one. */
    announced:
      "Each deletion is announced as it happens, and the running total is on the Diagnostics tab.",
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
    gigabytes: "{{count}} GB",
    never: "Never",
    /** A value the service accepts that this screen does not offer — set
     *  from the CLI. Shown as itself rather than snapped to a neighbour. */
    raw: "{{count}}",
  },
} as const;
