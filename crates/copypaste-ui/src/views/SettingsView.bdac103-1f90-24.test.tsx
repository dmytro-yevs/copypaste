/**
 * CopyPaste-bdac.103 — excluded-apps chips use bg-ide-elevated/40 (not bg-ide-bg)
 * CopyPaste-1f90.24  — tab indicator uses var(--ease) (not inline cubic-bezier)
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri bridge before importing SettingsView
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

// ---------------------------------------------------------------------------
// Daemon stub with an excluded-app pre-loaded
// ---------------------------------------------------------------------------
function makeInvokeWithExcludedApp(bundleId: string) {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.7.5");
    // Accessibility/notification permission checks (cmd may be obfuscated at runtime)
    if (cmd === "check_accessibility_permission" || cmd === "check_notification_permission")
      return Promise.resolve(true);

    const method = (args as { method?: string } | undefined)?.method;
    switch (method) {
      case "status":
        return Promise.resolve({
          ok: true,
          data: { ready: true, degraded: false, build_version: "0.7.5" },
          error: null, error_code: null,
        });
      case "get_config":
        return Promise.resolve({
          ok: true,
          data: {
            p2p_enabled: true, supabase_url: null, supabase_anon_key: null,
            relay_url: null, sync_enabled: true, sync_on_wifi_only: false,
            lan_visibility: true, auto_apply_synced_clip: true,
            collect_public_ip: false, paste_as_plain_text: false,
            // Pre-load one excluded app so chips render on the General tab
            excluded_app_bundle_ids: [bundleId],
            max_text_size_bytes: null, max_image_size_bytes: null,
            max_file_size_bytes: null, storage_quota_bytes: null,
            sensitive_ttl_secs: null,
          },
          error: null, error_code: null,
        });
      case "get_private_mode":
        return Promise.resolve({
          ok: true, data: { private_mode: false }, error: null, error_code: null,
        });
      case "get_sync_status":
        return Promise.resolve({
          ok: true, data: { last_sync_ms: null, supabase_url: null }, error: null, error_code: null,
        });
      default:
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
    }
  };
}

const BUNDLE_ID = "com.example.testapp";

beforeEach(() => {
  localStorage.clear();
  invoke.mockReset();
  invoke.mockImplementation(makeInvokeWithExcludedApp(BUNDLE_ID));
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// bdac.103: excluded-apps chips must carry bg-ide-elevated/40 (not bg-ide-bg)
//
// Strategy: the remove button inside each chip has aria-label="Remove <bundleId>".
// Find that button, get its parentElement (the chip span), and check the classes.
// ---------------------------------------------------------------------------
describe("CopyPaste-bdac.103: excluded-apps chips use bg-ide-elevated/40", () => {
  it("chip does NOT carry bg-ide-bg class", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for the remove button inside the chip to appear
    const removeBtn = await screen.findByRole("button", {
      name: new RegExp(`Remove ${BUNDLE_ID}`, "i"),
    }, { timeout: 5000 });

    // The chip is the button's direct parent span
    const chip = removeBtn.parentElement as HTMLElement;
    expect(chip, "Chip span (parent of remove button) must exist").toBeTruthy();

    expect(
      chip.classList.contains("bg-ide-bg"),
      "Chip must NOT carry bg-ide-bg (canvas colour — zero contrast vs panel background)"
    ).toBe(false);
  });

  it("chip carries bg-ide-elevated/40 class for contrast against canvas", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    const removeBtn = await screen.findByRole("button", {
      name: new RegExp(`Remove ${BUNDLE_ID}`, "i"),
    }, { timeout: 5000 });

    const chip = removeBtn.parentElement as HTMLElement;
    expect(chip, "Chip span must exist").toBeTruthy();

    // Tailwind Opacity modifier: the literal class name is "bg-ide-elevated/40"
    expect(
      chip.classList.contains("bg-ide-elevated/40"),
      "Chip must carry bg-ide-elevated/40 for skin-aware contrast against canvas"
    ).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 1f90.24: tab indicator must use var(--ease) not inline cubic-bezier
// ---------------------------------------------------------------------------
describe("CopyPaste-1f90.24: settings tab indicator uses --mo-ease-standard token", () => {
  it("tab indicator transition does not contain a hardcoded cubic-bezier string", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for settings UI to load (tabs visible = ready state)
    await screen.findByRole("tab", { name: /general/i });

    // The indicator is an aria-hidden <span> with .bg-ide-accent
    const indicator = document.querySelector(
      "span[aria-hidden='true'].bg-ide-accent"
    ) as HTMLElement | null;

    expect(indicator, "Tab indicator span[aria-hidden].bg-ide-accent must be present")
      .not.toBeNull();

    const transition = indicator!.style.transition ?? "";

    expect(
      transition.includes("cubic-bezier"),
      `Tab indicator transition must not use inline cubic-bezier, got: "${transition}"`
    ).toBe(false);

    expect(
      transition.includes("var(--ease)"),
      `Tab indicator must reference var(--ease), got: "${transition}"`
    ).toBe(true);
  });
});
