/**
 * "The clipboard service isn't running" — the offer, and how it degrades.
 *
 * There are two variants and both have to be honest:
 *
 *  - the bridge can start the service → a button that starts it;
 *  - it cannot (the command is not routed, or there is no bridge at all
 *    because this is a browser or a test) → the command to run, a button to
 *    copy it, and a way to re-check.
 *
 * What must never happen is a third variant: an error the user cannot act on,
 * or an empty list that reads as "you have copied nothing".
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { ServiceOffline } from "@/components/shell/ServiceOffline";
import { withUser } from "@/test/harness";

const startService = vi.fn();
const writeText = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    hasBridge: () => Boolean((window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__),
    startService: () => startService(),
  };
});

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: (text: string) => writeText(text),
}));

function withBridge() {
  (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
}

afterEach(() => {
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  startService.mockReset();
  writeText.mockReset();
  vi.restoreAllMocks();
});

describe("with a bridge that can start the service", () => {
  it("offers to start it", async () => {
    withBridge();
    startService.mockResolvedValue(undefined);
    const { user } = withUser(<ServiceOffline />);

    const button = screen.getByRole("button", { name: /start the service/i });
    await user.click(button);
    expect(startService).toHaveBeenCalledTimes(1);
  });
});

describe("with no way to start it", () => {
  it("shows the command to run and a way to copy it", () => {
    // No bridge at all — a browser, or this test.
    withUser(<ServiceOffline />);
    expect(screen.getByText("copypaste-daemon")).toBeTruthy();
    expect(screen.getByRole("button", { name: /copy command/i })).toBeTruthy();
  });

  it("falls back to the instruction when the command is not routed", async () => {
    // `start_service` is declared in ipc.ts but deliberately not implemented:
    // it needs a daemon-lifecycle decision. Until then the failure must turn
    // into guidance, not into an error message.
    withBridge();
    startService.mockRejectedValue("Command start_service not found");
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { user } = withUser(<ServiceOffline />);

    await user.click(screen.getByRole("button", { name: /start the service/i }));
    await waitFor(() => expect(screen.getByText("copypaste-daemon")).toBeTruthy());
    expect(screen.queryByText(/cannot do that yet/i)).toBeNull();
  });

  it("copies the command rather than a path", async () => {
    const { user } = withUser(<ServiceOffline />);
    await user.click(screen.getByRole("button", { name: /copy command/i }));
    expect(writeText).toHaveBeenCalledWith("copypaste-daemon");
    // INV-12: a command name is not a path, and nothing here discloses one.
    expect(writeText.mock.calls[0]?.[0]).not.toMatch(/\/Users\/|\/home\//);
  });

  it("never renders a filesystem path", () => {
    const { container } = withUser(<ServiceOffline />);
    expect(container.innerHTML).not.toMatch(/\/Users\/|\/home\/|\.sock/);
  });
});
