export const pairing = {
  heading: "Add a device",
  create: "Show pairing code",
  createHint: "Create and show a new pairing code in the native view",
  createHintWeb: "Create and show a new pairing code in this preview",
  join: "Scan pairing code",
  joinHint: "Scan a pairing code with this device",
  joinHintWeb: "Enter the pairing code shown by the other device",
  checking: "Checking for an active pairing…",
  cancelling: "Cancelling…",
  reviewSecure: "Review securely",
  progress: {
    checking: "Checking pairing status…",
    opening: "Opening protected pairing…",
  },
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
  semantic: {
    ready: {
      title: "Pair another device",
      body: "Pairing codes and security comparisons open in a protected native surface.",
    },
    waiting_for_peer: {
      title: "Waiting for the other device…",
      body: "Keep the protected pairing surface open on both devices.",
    },
    securing_connection: {
      title: "Establishing a private connection…",
      body: "CopyPaste is verifying the pairing request.",
    },
    compare_codes: {
      title: "Confirm on the protected pairing surface",
      body: "Compare the security code there. The code is never shown in this page.",
    },
    paired: {
      title: "Device paired",
      body: "The device is ready to sync.",
      device: "{{name}} is ready to sync.",
    },
    rejected: {
      title: "Pairing didn't match",
      body: "Nothing was paired. Start again when both devices are ready.",
    },
    cancelled: {
      title: "Pairing cancelled",
      body: "No device was added.",
    },
    timed_out: {
      title: "Pairing code expired",
      body: "Start again to create a new protected pairing session.",
    },
    code_mismatch: {
      title: "Pairing didn't match",
      body: "The security codes did not match. No device was paired.",
    },
    incompatible_version: {
      title: "Pairing couldn't finish",
      body: "The other device uses an incompatible version. No device was paired.",
    },
    unreachable: {
      title: "Pairing couldn't finish",
      body: "Could not reach the other device. Check that both devices are on the same network.",
    },
    busy: {
      title: "Pairing is busy",
      body: "Another pairing is already in progress.",
    },
    failed: {
      title: "Pairing failed",
      body: "CopyPaste couldn't finish pairing.",
    },
  },
} as const;
