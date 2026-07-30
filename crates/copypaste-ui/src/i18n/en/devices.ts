export const devices = {
  title: "Devices",

  /** One sentence, two dialogs: both halves of pairing fail the same way for
   *  the same reason, and two copies would drift. */
  pairUnavailable:
    "Pairing isn't available in this build yet. It runs in the background service, which this build doesn't reach.",

  unavailable: {
    title: "Device sync isn't available in this build",
    body: "Pairing runs in the background service, which this build doesn't reach yet. Everything else on this device works normally.",
  },

  actions: {
    pairNew: "Pair a new device",
    add: "Add a device",
    syncAll: "Sync now",
    syncing: "Syncing…",
    syncAllHint: "Sync with every paired device now",
  },

  loading: { title: "Loading…", body: "Looking for paired devices." },
  failed: { title: "Couldn't load your devices" },
  none: {
    title: "No other devices paired",
    body: "Pair your phone or another Mac to sync your clipboard — end-to-end encrypted, directly between your devices.",
  },

  /** INV-15: `{{name}}` is what the other device called itself, never a
   *  verified identity — `nameHint` is what says so. */
  peer: {
    thisDevice: "this device",
    nameHint: "Name reported by the device itself — not verified",
    lastSeen: "Last seen {{age}}",
    online: "On the network",
    offline: "Not seen",
    syncOne: "Sync with {{name}} now",
    syncOneHint: "Sync with this device now",
    unpairOne: "Unpair {{name}}",
    unpairHint: "Unpair this device",
    revokeOne: "Revoke {{name}}",
    revokeHint: "Cut this device off for good — for a device you no longer have",
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
   * back (CLAUDE.md rule 4). The three consequences are separate entries
   * because a screen may show them as a list and a confirmation has to name
   * what is lost rather than say "are you sure".
   */
  revoke: {
    title: "Revoke {{name}}?",
    body: "This can't be undone from this device.",
    lostCode:
      "The pairing code stops working for good. Pairing these devices again means generating a new code and entering it by hand.",
    lostOneSided:
      "{{name}} isn't told, and keeps its own half of the pairing. If you have other devices, revoke it there too.",
    keptHistory: "Nothing already synced is deleted, here or on that device.",
    confirmLabel: "I understand this can't be undone",
    action: "Revoke",
  },

  toast: {
    paired: "Device paired",
    unpaired: "Unpaired {{name}}",
    revoked: "Revoked {{name}} — its pairing code can never be used again",
    nothingToSync: "Nothing to sync — no paired devices",
    synced_one: "Synced {{count}} device — sent {{sent}}, received {{received}}",
    synced_other:
      "Synced {{count}} devices — sent {{sent}}, received {{received}}",
    syncedPartial: "Synced {{done}} of {{total}} devices — {{failed}} failed",
  },

  create: {
    title: "Pair a new device",
    description:
      "Generate a code here, then scan it with the other device — or enter it by hand under Devices → Add a device.",
    nameLabel: "Name for the other device",
    defaultName: "unnamed device",
    nameHint: "Used until that device tells us its own name.",
    qrLabel: "Pairing code as a QR code. Scan it with the other device.",
    qrHint:
      "On the other device, choose Devices → Add a device and scan this. It carries the code and the address together.",
    codeLabel: "Pairing code",
    codeCameraNote: "(if the other device has no camera)",
    codeHidden: "Pairing code, hidden. Activate reveal to show it.",
    revealLabel: "Reveal the pairing code",
    reveal: "Click to reveal",
    codeWarning:
      "Anyone with this code can pair with this device. It is shown once and cannot be retrieved again — generate a new one if you lose it.",
    addressLabel: "This device's address",
    addressMissing: "Not reachable — check that sync is enabled",
    addressHint: "Enter this alongside the code on the other device.",
    copyCode: "Copy code",
    copied: "Copied",
    copyFailed: "The code couldn't be copied. Read it off the screen instead.",
    generate: "Generate code",
    regenerate: "Generate a new code",
    generating: "Generating…",
  },

  accept: {
    title: "Add a device",
    description:
      "Scan the code shown on the other device, or enter it by hand.",
    startingCamera: "Starting the camera…",
    scan: "Scan a code",
    nearby: "On this network",
    nearbyHint:
      "Unverified — these names are reported by the devices themselves and are only confirmed once pairing succeeds. Choosing one fills in the address; the code still has to come from that device.",
    rescan: "Look for nearby devices",
    rescanning: "Looking…",
    codeLabel: "Pairing code",
    addrLabel: "Address",
    addrPlaceholder: "192.168.1.24:7420",
    addrHint: "Shown on the other device under the code, as host:port.",
    retryHint:
      "Nothing was changed — check the code and the address and try again.",
    action: "Pair",
    pairing: "Pairing…",
  },

  scanner: {
    unavailable: "No camera is available, so enter the code below instead.",
    videoLabel: "Camera, looking for a pairing code",
    starting: "Starting the camera…",
    scanning: "Point the camera at the code on the other device.",
  },

  qr: { failed: "The code couldn't be drawn. Read it out instead." },
} as const;
