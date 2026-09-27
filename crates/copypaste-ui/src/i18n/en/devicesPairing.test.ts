import { describe, expect, it } from "vitest";

import { devices } from "./devices";
import { pairing } from "./devicesPairing";

const PAIRING_SNAPSHOT = {
  heading: "Add a device", create: "Show pairing code", createHint: "Create and show a new pairing code in the native view", createHintWeb: "Show safe pairing status in this preview.", join: "Scan pairing code", joinHint: "Scan a pairing code with this device", joinHintWeb: "Pairing details are available only in the protected native app.", checking: "Checking for an active pairing…", cancelling: "Cancelling…", reviewSecure: "Review securely",
  progress: { checking: "Checking pairing status…", opening: "Opening protected pairing…" },
  presentationUnavailable: "The protected pairing view didn't open. Try Show details, or cancel and start again.", previewUnavailable: "This preview shows safe pairing status only. Pairing details stay in the protected native app.", scanCancelled: "No pairing code was scanned. You can try again when you're ready.", decisionSubmitted: "Your decision was sent. Waiting for the other device to finish…", present: "Show details", presenting: "Opening…", cancel: "Cancel pairing", reject: "Doesn't match", rejecting: "Rejecting…", rejectLabel: "Codes don't match — reject pairing", confirm: "Codes match", confirming: "Confirming…", confirmLabel: "Codes match — confirm pairing in the native view", inviteTitle: "Pair a new device", inviteBody: "Scan this QR code from CopyPaste on the other device, or enter the code and address there.", reveal: "Click to reveal", revealLabel: "Click to reveal QR code", expires: "Expires in {{count}} seconds", codeLabel: "Pairing code", addressLabel: "Pairing address", joinTitle: "Enter pairing code", joinBody: "Use the code and address shown by the other device.", joinCode: "Pairing code", joinAddress: "Pairing address", joinAction: "Start pairing",

} as const;

describe("devices pairing catalogue", () => {
  it("keeps devices.pairing as the extracted catalogue object", () => {
    expect(devices.pairing).toBe(pairing);
  });

  it("keeps the control catalogue without duplicating native state copy", () => {
    expect(pairing).toEqual(PAIRING_SNAPSHOT);
    expect(pairing).not.toHaveProperty("semantic");
  });
});
