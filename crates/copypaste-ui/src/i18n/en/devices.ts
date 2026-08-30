export const devices = {
  title: "Devices",

  pairing: {
    heading: "Add a device",
    create: "Show pairing code",
    createHint: "Create and show a new pairing code in the native view",
    createHintWeb: "Create and show a new pairing code in this preview",
    join: "Scan pairing code",
    joinHint: "Scan a pairing code with this device",
    joinHintWeb: "Enter the pairing code shown by the other device",
    checking: "Checking for an active pairing…",
    presentationUnavailable:
      "The protected pairing view didn't open. Try Show details, or cancel and start again.",
    scanCancelled: "No pairing code was scanned. You can try again when you're ready.",
    decisionSubmitted:
      "Your decision was sent. Waiting for the other device to finish…",
    present: "Show details",
    presenting: "Opening…",
    cancel: "Cancel pairing",
    reject: "Doesn't match",
    rejecting: "Rejecting…",
    rejectLabel: "Codes don't match — reject pairing",
    confirm: "Codes match",
    confirming: "Confirming…",
    confirmLabel: "Codes match — confirm pairing in the native view",
    inviteTitle: "Pair a new device",
    inviteBody: "Scan this QR code from CopyPaste on the other device, or enter the code and address there.",
    reveal: "Click to reveal",
    revealLabel: "Click to reveal QR code",
    expires: "Expires in {{count}} seconds",
    codeLabel: "Pairing code",
    addressLabel: "Pairing address",
    joinTitle: "Enter pairing code",
    joinBody: "Use the code and address shown by the other device.",
    joinCode: "Pairing code",
    joinAddress: "Pairing address",
    joinAction: "Start pairing",
    state: {
      idle: {
        title: "Ready to pair",
        body: "Show a code for another device, or scan the code shown there.",
      },
      waiting: {
        title: "Waiting for the other device",
        body: "The pairing code is shown only in the protected native view.",
      },
      handshaking: {
        title: "Connecting securely",
        body: "Keep both devices nearby while they work out a shared security code.",
      },
      confirm: {
        title: "Compare the security codes",
        body: "Open the protected view on both devices. Confirm only when the codes match exactly.",
      },
      confirmed: {
        title: "Device paired",
        body: "The device is now in your paired-device list.",
        device: "{{name}} is now paired and ready to sync.",
      },
      rejected: {
        title: "Pairing rejected",
        body: "The codes didn't match, so neither device was saved.",
      },
      cancelled: {
        title: "Pairing cancelled",
        body: "Neither device was saved. You can start again when you're ready.",
      },
      timedOut: {
        title: "Pairing timed out",
        body: "Keep both devices on the same network, show a fresh code, and try again.",
      },
      failed: {
        title: "Pairing couldn't finish",
        body: "No device was saved. Check both devices and try again.",
      },
    },
  },

  unavailable: {
    title: "Device sync unavailable",
  },

  actions: {
    syncAll: "Sync now",
    syncing: "Syncing…",
    syncAllHint: "Sync with every paired device now",
  },

  detail: {
    heading: "Device information",
    close: "Close device details",
    status: "Status",
    platform: "Platform",
    listenAddress: "Listen address",
    protocol: "Sync protocol",
    captureBackend: "Capture backend",
    pairing: "Pairing",
    notPaired: "Not paired",
    trust: "Trust",
    transport: "Transport",
    notAvailable: "Not available",
    notReported: "Not reported by this device",
    checking: "Checking…",
    thisDevice: "This device",
    peerName: "Device name is self-reported",
    discoveryName: "Network discovery is unverified",
    lastManualSync: "Last sync requested here",
    syncRoundDuration: "Sync round duration",
    durationMs: "{{duration}} ms",
    notMeasured: "Not measured yet",
    transfer: "Items moved in that round",
    transferCounts: "{{sent}} sent · {{received}} received",
    noManualSync: "No sync requested here yet",
    pairedHere: "Paired on this device",
    directTransport: "Direct encrypted peer-to-peer",
    peerPlatform: "Not reported by peer",
    peerProtocol: "Not reported by peer",
    seen: "Seen on this network",
    notSeen: "Not currently discovered",
    advertisedAddress: "Advertised address (unverified)",
    discoveryTrust: "Unverified network advertisement",
    discoveryTransport: "Local network discovery",
    discoveryPlatform: "Not reported by advertisement",
    discoveryProtocol: "Not reported by advertisement",
  },

  own: {
    heading: "This device",
    name: "This device",
    loading: "Checking this device…",
    checking: "Checking",
    unavailableLabel: "Unavailable",
    unavailable: "Device details are unavailable.",
    version: "App version",
    history: "Local history",
    items_one: "{{count}} item",
    items_other: "{{count}} items",
    capture: "Clipboard capture",
    captureOn: "Recording",
    captureOff: "Paused",
    privateMode: "Private mode",
    privateModeOn: "On",
    privateModeOff: "Off",
    clipboard: "Clipboard connection",
    clipboardConnected: "Connected through the background service",
    rename: {
      open: "Rename device",
      title: "Rename this device",
      label: "Device name",
      description: "Used to identify this device during sync and network discovery.",
      action: "Rename",
      saving: "Saving…",
      saved: "Device name saved",
    },
  },

  paired: {
    heading: "Paired devices",
    online_one: "{{count}} on this network",
    online_other: "{{count}} on this network",
    listLabel: "Paired devices",
  },

  discovered: {
    heading: "Discovered on your network",
    refresh: "Refresh",
    refreshLabel: "Refresh devices discovered on this network",
    loading: "Looking for devices on this network…",
    unavailable: "Network discovery is unavailable.",
    failed: "Devices on this network couldn't be checked. Try Refresh.",
    none: "No devices found",
    listLabel: "Devices discovered on this network",
    unverified: "Unverified device details reported over the network",
    paired: "Paired",
    address: "Reported address",
    lastSeen: "Last discovered",
  },

  loading: { title: "Loading…", body: "Looking for paired devices." },
  failed: { title: "Couldn't load your devices" },
  none: {
    title: "No other devices paired",
    body: "Pair another Mac or Android device to keep its clipboard history in sync.",
  },

  /** The cap is a refusal, never an eviction, so the only remedy is one the
   *  user carries out. Saying so before the code is minted is the difference
   *  between a sentence and a failed pairing. */
  cap: {
    count_one: "{{count}} device paired · {{max}} maximum",
    count_other: "{{count}} devices paired · {{max}} maximum",
    full: "This device is paired with {{max}} others, the most it can hold. Unpair or revoke one before pairing another.",
    hint: "Unpair a device first",
  },

  /**
   * What a device is doing, in the terms the daemon can actually answer.
   *
   * Six states rather than a dot, because the remedies differ: a code nobody
   * has used, a device this one holds no address for, and a device sitting on
   * the network not syncing are three different problems that a green/grey
   * badge reported identically.
   *
   * Every `label` is a word, not only a colour (manifest 06, A11Y).
   */
  state: {
    synced: { label: "In sync" },
    away: {
      label: "Away",
      hint: "Not on this network right now. It catches up the next time both devices are on the same network.",
    },
    stalled: {
      label: "Not syncing",
      hint: "It's on this network, but nothing has synced for a while. Try Sync now — if that fails, check the other device is awake and CopyPaste is running on it.",
    },
    inbound: {
      label: "Incoming only",
      hint: "This device has no address for it, so only that device can start a sync. One successful Sync now while it's on this network settles that for good.",
    },
    waiting: {
      label: "Not connected yet",
      hint: "The pairing code hasn't been used. Enter it on the other device — a code stops working a few minutes after it's created, so generate a new one if it has been longer.",
    },
    /** `hint` is the fallback for the two error kinds whose own sentence says
     *  only "the background service returned an error". */
    failing: {
      label: "Sync failed",
      hint: "The last sync with this device didn't finish. Check it's awake and on this network, then try again.",
    },
  },

  /** INV-15: `{{name}}` is what the other device called itself, never a
   *  verified identity — `nameHint` is what says so. */
  peer: {
    thisDevice: "this device",
    nameHint: "Name reported by the device itself — not verified",
    nameUnverified: "Device name is self-reported",
    /** `last_seen_ms` is written at the end of a successful session, so it is
     *  a last-*synced* time. Rendering the `0` an uncontacted pairing carries
     *  through a relative formatter read "Last seen 56 years ago". */
    lastSynced: "Last synced {{age}}",
    neverSynced: "Never synced",
    lastSyncLabel: "Last successful sync",
    networkLabel: "Network discovery",
    addressLabel: "Connection address",
    /** `lastSynced` comes from the daemon's own cadence; these two are the runs
     *  this screen started, which is the only place a per-peer outcome exists.
     *  `sent`/`received` are what separates a sync that worked from one that
     *  merely completed. */
    lastRun: "Last sync from here {{age}} · sent {{sent}}, received {{received}}",
    failedAt: "Sync failed {{age}}",
    retryOne: "Try syncing with {{name}} again",
    online: "On this network",
    offline: "Not seen on this network",
    syncOne: "Sync with {{name}} now",
    syncOneHint: "Sync with this device now",
    syncAction: "Sync now",
    unpairOne: "Unpair {{name}}",
    unpairHint: "Unpair this device",
    unpairAction: "Unpair",
    revokeOne: "Revoke {{name}}",
    revokeHint: "Permanently block this pairing from peer sync on this device",
    revokeAction: "Revoke",
  },

  /** The lighter of the two. It says what unpairing does *not* cost, because
   *  that is what tells the two apart at the moment of choosing. */
  unpair: {
    title: "Unpair {{name}}?",
    body: "This device will stop syncing with it. Unpairing is one-sided — the other device keeps its half of the pairing until it unpairs too. Nothing already synced is deleted, and you can pair these devices again later.",
    lost: "No longer have this device? Revoke it instead, so its pairing code can never be used again.",
    action: "Unpair",
  },

  /**
   * The heavier one, and every line of it is something the user does not get
   * back (AGENTS.md rule 4). The three consequences are separate entries
   * because a screen may show them as a list and a confirmation has to name
   * what is lost rather than say "are you sure".
   */
  revoke: {
    title: "Revoke {{name}}?",
    body: "This can't be undone from this device. It blocks this peer pairing only; this app has no command to rotate or revoke a cloud sync key.",
    lostCode:
      "The pairing code stops working for good. Pairing these devices again means generating a new code and entering it by hand.",
    lostOneSided:
      "{{name}} isn't told, and keeps its own half of the pairing. If you have other devices, revoke it there too.",
    keptHistory: "Nothing already synced is deleted, here or on that device.",
    confirmLabel: "I understand this can't be undone",
    action: "Revoke",
  },

  toast: {
    unpaired: "Unpaired {{name}}",
    revoked:
      "Revoked {{name}} from peer sync — its pairing code can never be used again",
    nothingToSync: "Nothing to sync — no paired devices",
    synced_one: "Synced {{count}} device — sent {{sent}}, received {{received}}",
    synced_other:
      "Synced {{count}} devices — sent {{sent}}, received {{received}}",
    syncedPartial: "Synced {{done}} of {{total}} devices — {{failed}} failed",
    syncedWithSizeRefusals_one:
      "Synced {{count}} device — sent {{sent}}, received {{received}}; oversized-item refusals: {{skipped}}",
    syncedWithSizeRefusals_other:
      "Synced {{count}} devices — sent {{sent}}, received {{received}}; oversized-item refusals: {{skipped}}",
    syncedWithKnownSizeRefusals_one:
      "Synced {{count}} device — sent {{sent}}, received {{received}}; oversized-item refusals: {{skipped}}, and counts are unavailable for one or more peer syncs",
    syncedWithKnownSizeRefusals_other:
      "Synced {{count}} devices — sent {{sent}}, received {{received}}; oversized-item refusals: {{skipped}}, and counts are unavailable for one or more peer syncs",
    syncedWithUnknownSizeRefusals_one:
      "Synced {{count}} device — sent {{sent}}, received {{received}}; oversized-item refusal counts are unavailable for one or more peer syncs",
    syncedWithUnknownSizeRefusals_other:
      "Synced {{count}} devices — sent {{sent}}, received {{received}}; oversized-item refusal counts are unavailable for one or more peer syncs",
  },

} as const;
