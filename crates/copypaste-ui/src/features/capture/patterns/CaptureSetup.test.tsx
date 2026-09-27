import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { captureSnapshot, withClient } from "@/test/harness";
import { CaptureSetup } from "./CaptureSetup";

describe("CaptureSetup", () => {
  it("keeps setup instructions and Android notice options collapsed", () => {
    const { container } = withClient(<CaptureSetup snapshot={captureSnapshot()} />);
    const disclosures = container.querySelectorAll("details");
    expect(disclosures.length).toBe(2);
    for (const disclosure of disclosures) expect(disclosure.open).toBe(false);
    expect(screen.getByRole("button", { name: "Save now" })).toBeTruthy();
    fireEvent.click(screen.getByText("Setup instructions"));
    expect(disclosures[0].open).toBe(true);
  });

  it("shows the user's enabled preference even when capture needs setup", () => {
    const snapshot = captureSnapshot({
      health: { state: "not_granted", reason: "not_installed" },
      nextStep: "install_shizuku",
      shizuku: { ...captureSnapshot().shizuku, enabled: false },
    });
    withClient(<CaptureSetup snapshot={snapshot} />);
    expect(screen.getByRole("switch", { name: "Capture from other apps" }).getAttribute("aria-checked")).toBe("false");
  });

  it("uses the shared warning notice for dropped captures", () => {
    withClient(
      <CaptureSetup
        snapshot={captureSnapshot({ rung: "desktop", droppedClips: 2 })}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain(
      "2 copies were captured but couldn't be saved.",
    );
  });

  it("uses the shared assertive fault semantics for a refused read", () => {
    withClient(
      <CaptureSetup
        snapshot={captureSnapshot({
          rung: "desktop",
          health: {
            state: "granted_not_working",
            reason: "read_refused",
          },
          headline: "Clipboard access was refused.",
          detail: "Copy once, then try again.",
        })}
      />,
    );

    expect(screen.getByRole("alert").getAttribute("aria-live")).toBe(
      "assertive",
    );
  });

  it("keeps setup states polite when no capture failure occurred", () => {
    withClient(
      <CaptureSetup
        snapshot={captureSnapshot({
          rung: "desktop",
          health: { state: "disabled" },
          headline: "Background capture is off.",
        })}
      />,
    );

    expect(screen.getByRole("status").getAttribute("aria-live")).toBe(
      "polite",
    );
  });
});
