import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { GeneralTab, type GeneralTabProps } from "./GeneralTab";
import type { AppSettings } from "../../../lib/ipc";
import { DEFAULT_PREFS } from "../../../store";

// ---------------------------------------------------------------------------
// GeneralTab — "Allow screenshots" advisory placement (CopyPaste-7w060.1)
//
// Regression coverage: the long conditional advisory used to live inside the
// row's `.ctl` (flex-nowrap) div alongside the Toggle, which pushed the row
// title/icon into wrapping at the panel's usual width. It must render as a
// full-width sibling block after the row instead, mirroring the sync_enabled
// stub warning pattern.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.reject(new Error("unavailable in test"))),
}));

vi.mock("../../../lib/ipc", () => ({
  api: { setConfig: vi.fn(() => Promise.resolve()) },
  appVersion: vi.fn(() => Promise.reject(new Error("unavailable in test"))),
  invoke: vi.fn(() => Promise.reject(new Error("unavailable in test"))),
}));

function baseProps(overrides: Partial<GeneralTabProps> = {}): GeneralTabProps {
  return {
    offline: false,
    loadState: "ready",
    prefs: DEFAULT_PREFS,
    setPrefs: vi.fn(),
    syncEnabled: true,
    syncEnabledStub: false,
    privateMode: false,
    privateModeError: null,
    notifPermDenied: false,
    collectPublicIp: false,
    setCollectPublicIp: vi.fn(),
    pasteAsPlainText: false,
    setPasteAsPlainText: vi.fn(),
    allowScreenshots: true,
    allowScreenshotsError: null,
    excludedApps: [],
    newExcludedApp: "",
    setNewExcludedApp: vi.fn(),
    daemonVersion: "1.0.0",
    limitsMsg: {},
    buildConfigPatch: (overrides: Partial<AppSettings>) => overrides as AppSettings,
    handleSyncEnabledToggle: vi.fn(),
    handlePrivateMode: vi.fn(),
    handleAllowScreenshots: vi.fn(),
    addExcludedApp: vi.fn(),
    removeExcludedApp: vi.fn(),
    setReloadKey: vi.fn(),
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("GeneralTab — Allow screenshots advisory", () => {
  it("renders the advisory note outside the row's .ctl control column", () => {
    render(<GeneralTab {...baseProps()} />);

    const note = screen.getByRole("note");
    expect(note).toHaveTextContent(
      "Clipboard content may be captured by screenshots and screen recordings.",
    );

    // The note must NOT live inside the row's .ctl (nowrap flex row that
    // also holds the Toggle) — that composition is what dragged the title
    // and info icon into wrapping.
    expect(note.closest(".ctl")).toBeNull();

    // It must sit as a full-width sibling, not inside the two-column
    // .srow__c control cell either.
    expect(note.closest(".srow__c")).toBeNull();
  });

  it("omits the advisory note when allowScreenshots is off", () => {
    render(<GeneralTab {...baseProps({ allowScreenshots: false })} />);
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
  });
});
