/**
 * "The clipboard service isn't running" — the offer, and the four states behind
 * it (ADR-0004).
 *
 * The app owns the service's lifetime and ships it inside its own bundle, so
 * the recovery is a button rather than a command to type. What must never
 * happen is a screen with no action on it, an error the user cannot act on, or
 * an empty list that reads as "you have copied nothing".
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { ServiceOffline } from "@/components/shell/ServiceOffline";
import type { ServiceState } from "@/lib/ipc";
import { withUser } from "@/test/harness";

const serviceState = vi.fn();
const startService = vi.fn();
const restartService = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    serviceState: () => serviceState(),
    startService: () => startService(),
    restartService: () => restartService(),
  };
});

const STOPPED: ServiceState = { state: "stopped" };

beforeEach(() => {
  serviceState.mockReset().mockResolvedValue(STOPPED);
  startService.mockReset().mockResolvedValue(STOPPED);
  restartService.mockReset().mockResolvedValue(STOPPED);
});

afterEach(() => vi.restoreAllMocks());

describe("the service is installed but not running", () => {
  it("offers to start it, and starting it is one press", async () => {
    const { user } = withUser(<ServiceOffline />);
    await user.click(screen.getByRole("button", { name: /start the service/i }));
    expect(startService).toHaveBeenCalledTimes(1);
  });

  /**
   * The screen used to print `copypaste-daemon` and a "copy command" button.
   * ADR-0004 removed the reason for that: telling a user to open a terminal is
   * advice for a button that is already in front of them.
   */
  it("does not tell the user to run a terminal command", async () => {
    const { container } = withUser(<ServiceOffline />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /start the service/i })).toBeTruthy(),
    );
    expect(container.textContent).not.toContain("copypaste-daemon");
    expect(screen.queryByRole("button", { name: /copy command/i })).toBeNull();
  });

  it("reports a refusal as a sentence rather than leaving the button spinning", async () => {
    startService.mockRejectedValue("Command start_service not found");
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { user } = withUser(<ServiceOffline />);

    await user.click(screen.getByRole("button", { name: /start the service/i }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBeTruthy();
    // INV-30: the busy flag is released whatever happened, so the button is
    // pressable again.
    expect(
      screen.getByRole("button", { name: /start the service/i }),
    ).not.toHaveProperty("disabled", true);
  });
});

describe("the service is a different version", () => {
  /** The upgrade case (parity finding 2): the app was replaced and an older
   *  service still holds the socket. */
  it("says so, and offers a restart when this app started it", async () => {
    serviceState.mockResolvedValue({
      state: "running",
      version: "0.9.9",
      matches_app: false,
      ours: true,
    } satisfies ServiceState);
    const { user } = withUser(<ServiceOffline />);

    await waitFor(() =>
      expect(screen.getByText(/out of date/i)).toBeTruthy(),
    );
    await user.click(screen.getByRole("button", { name: /restart the service/i }));
    expect(restartService).toHaveBeenCalledTimes(1);
  });

  /** ADR-0004's asymmetry, seen by the user: the app does not kill processes
   *  it did not start, so the copy has to say what to do instead. */
  it("explains rather than promising when the service is not ours", async () => {
    serviceState.mockResolvedValue({
      state: "running",
      version: "0.9.9",
      matches_app: false,
      ours: false,
    } satisfies ServiceState);
    withUser(<ServiceOffline />);

    await waitFor(() =>
      expect(screen.getByText(/can't stop it/i)).toBeTruthy(),
    );
  });
});

describe("the build has no service to start", () => {
  it("says so instead of offering a button that cannot work", async () => {
    serviceState.mockResolvedValue({ state: "not_installed" } satisfies ServiceState);
    withUser(<ServiceOffline />);

    await waitFor(() =>
      expect(screen.getByText(/no background service/i)).toBeTruthy(),
    );
    expect(screen.queryByRole("button", { name: /start the service/i })).toBeNull();
    expect(screen.getByRole("button", { name: /check again/i })).toBeTruthy();
  });
});

describe("in every state", () => {
  it.each([
    ["stopped", STOPPED],
    ["not_installed", { state: "not_installed" } as ServiceState],
    [
      "version mismatch",
      { state: "running", version: "0.0.1", matches_app: false, ours: false } as ServiceState,
    ],
  ])("never renders a filesystem path (%s)", async (_name, state) => {
    serviceState.mockResolvedValue(state);
    const { container } = withUser(<ServiceOffline />);
    await waitFor(() => expect(container.textContent).toBeTruthy());
    expect(container.innerHTML).not.toMatch(/\/Users\/|\/home\/|\.sock/);
  });
});
