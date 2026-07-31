/**
 * The panel that exists because nothing surfaced any of this.
 *
 * Two of these tests are the point of the screen rather than checks on it: a
 * diagnostics report is written to be pasted into a public issue, so a
 * filesystem path in it is the local username in it (INV-12), and a clipping in
 * it is the whole sensitive-content rule undone in one gesture.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { DiagnosticsTab } from "@/components/settings/DiagnosticsTab";
import type { Diagnostics } from "@/service/diagnostics";
import { withUser } from "@/test/harness";

const getDiagnostics = vi.fn();
const copyText = vi.fn();

vi.mock("@/service/diagnostics", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("@/service/diagnostics")>();
  return { ...actual, getDiagnostics: () => getDiagnostics() };
});

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, copyText: (text: string) => copyText(text) };
});

function diagnostics(over: Partial<Diagnostics> = {}): Diagnostics {
  return {
    app_version: "2.0.0-alpha.1",
    app_protocol_version: 1,
    os: "macos",
    arch: "aarch64",
    service: {
      state: "running",
      version: "2.0.0-alpha.1",
      matches_app: true,
      ours: true,
    },
    status: {
      version: "2.0.0-alpha.1",
      protocol_version: 1,
      item_count: 42,
      capture_running: true,
      clipboard_backend: "macos-pasteboard",
      legacy_history_present: false,
      counters: {
        rejected_too_large: 3,
        lost_intermediates: 1,
        sensitive_swept: 2,
        index_purged: 5,
        uptime_secs: 12_045,
      },
    },
    report: [
      "CopyPaste diagnostics",
      "app: 2.0.0-alpha.1",
      "platform: macos aarch64",
      "service: running 2.0.0-alpha.1",
      "capture: running",
      "backend: macos-pasteboard",
      "items: 42",
      "refused-too-large: 3",
      "",
    ].join("\n"),
    ...over,
  };
}

beforeEach(() => {
  getDiagnostics.mockReset().mockResolvedValue(diagnostics());
  copyText.mockReset().mockResolvedValue(undefined);
});

afterEach(() => vi.restoreAllMocks());

describe("what is running", () => {
  it("says the service is running, and on which version", async () => {
    withUser(<DiagnosticsTab />);
    // "Running" is both the service badge and the capture badge — the panel
    // answers the two questions separately on purpose.
    expect((await screen.findAllByText("Running")).length).toBe(2);
    expect(screen.getAllByText(/CopyPaste 2\.0\.0-alpha\.1/).length).toBeGreaterThan(0);
  });

  it("still reports when the service is not answering at all", async () => {
    getDiagnostics.mockResolvedValue(
      diagnostics({
        service: { state: "stopped" },
        status: null,
        report: "CopyPaste diagnostics\nservice: stopped\nstatus: not answering\n",
      }),
    );
    withUser(<DiagnosticsTab />);
    expect(await screen.findByText("Stopped")).toBeTruthy();
    expect(screen.getByText(/not answering/)).toBeTruthy();
  });

  /** An adopted daemon on another version is the upgrade case, and the two
   *  facts are different: one explains the breakage, the other explains why
   *  this app cannot fix it. */
  it("names both an adopted service and a version difference", async () => {
    getDiagnostics.mockResolvedValue(
      diagnostics({
        service: {
          state: "running",
          version: "1.9.0",
          matches_app: false,
          ours: false,
        },
      }),
    );
    withUser(<DiagnosticsTab />);
    expect(await screen.findByText(/different version from this app/)).toBeTruthy();
    expect(screen.getByText(/can't restart it/)).toBeTruthy();
  });

  it("flags a test backend rather than letting it pass for the real one", async () => {
    getDiagnostics.mockResolvedValue(
      diagnostics({
        status: { ...diagnostics().status!, clipboard_backend: "fake" },
      }),
    );
    withUser(<DiagnosticsTab />);
    const badge = await screen.findByText("fake");
    expect(badge.className).toContain("warn");
  });
});

describe("what has been dropped", () => {
  it("shows every count, including the ones nothing surfaced before", async () => {
    const { container } = withUser(<DiagnosticsTab />);
    await screen.findByText(/Copies refused for being too large/);
    expect(screen.getByText(/replaced before they could be read/)).toBeTruthy();
    expect(screen.getByText(/Detected secrets deleted automatically/)).toBeTruthy();
    expect(screen.getByText(/Search entries removed at startup/)).toBeTruthy();
    expect(container.textContent).toContain("3");
  });

  /** A row that only appears when it is non-zero is one nobody knows to look
   *  for, so "nothing was dropped" has to be a visible answer. */
  it("renders a zero rather than hiding the row", async () => {
    getDiagnostics.mockResolvedValue(
      diagnostics({
        status: {
          ...diagnostics().status!,
          counters: {
            rejected_too_large: 0,
            lost_intermediates: 0,
            sensitive_swept: 0,
            index_purged: 0,
            uptime_secs: 60,
          },
        },
      }),
    );
    const { container } = withUser(<DiagnosticsTab />);
    await screen.findByText(/Copies refused for being too large/);
    expect(container.textContent).toContain("0");
  });
});

describe("the copyable report", () => {
  it("shows the report before offering to copy it", async () => {
    withUser(<DiagnosticsTab />);
    expect(await screen.findByText(/refused-too-large: 3/)).toBeTruthy();
  });

  it("copies exactly what it displayed, through the backend", async () => {
    const { user } = withUser(<DiagnosticsTab />);
    await user.click(await screen.findByRole("button", { name: /Copy report/ }));
    await waitFor(() => expect(copyText).toHaveBeenCalledTimes(1));
    expect(copyText.mock.calls[0]![0]).toBe(diagnostics().report);
  });

  /**
   * **The test that matters**, in its half of the enforcement.
   *
   * The bridge scrubs every free-text field with
   * `copypaste_ipc::redact::scrub_paths` before the payload crosses — the
   * adversarial version of this, with a socket path in each of them, is
   * `service::diagnostics::tests::no_path_survives_anywhere_on_the_payload`.
   * Repeating it here would mean a second redactor on this side, which is the
   * duplication rule 1 exists to stop. What this asserts is the other half:
   * that the screen adds nothing path-shaped of its own, and neither authors
   * nor rebuilds the block it copies.
   */
  it("renders no filesystem path anywhere", async () => {
    const { container } = withUser(<DiagnosticsTab />);
    await screen.findByRole("button", { name: /Copy report/ });
    expect(container.textContent ?? "").not.toMatch(
      /\/Users\/|\/home\/|~\/|\.sock|[A-Za-z]:\\|\$HOME/,
    );
  });

  /**
   * The other one. Nothing on this screen reads an item, so a clipping can only
   * arrive by someone adding a field that carries one — which is what this
   * fails on.
   */
  it("renders no clipboard content even when the service sends some", async () => {
    const clipping = "sk-live-4f9c1d2e-THIS-IS-A-CLIPPING";
    getDiagnostics.mockResolvedValue(
      diagnostics({
        app_version: clipping,
        status: { ...diagnostics().status!, clipboard_backend: clipping },
        report: `CopyPaste diagnostics\napp: 2.0.0-alpha.1\n`,
      }),
    );
    const { container } = withUser(<DiagnosticsTab />);
    await screen.findByRole("button", { name: /Copy report/ });
    // The backend name is the one daemon-authored string this screen prints,
    // and it is a backend name — not an item. Nothing else may echo it.
    expect(
      (container.textContent ?? "").split(clipping).length - 1,
    ).toBeLessThanOrEqual(1);
  });
});
