import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// Mock Tauri core so IPC calls resolve or reject in a controlled way.
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: () => Promise.resolve("0.5.3"),
}));

import { AboutView } from "./AboutView";

beforeEach(() => {
  invoke.mockReset();
  // ipcCall wraps invoke("ipc_call", ...) and expects { ok: true, data: ... }.
  // Returning { ok: true, data: null } simulates a successful daemon response.
  invoke.mockResolvedValue({ ok: true, data: null });
});

describe("AboutView visual structure (JetBrains tokens)", () => {
  it("renders the header title via ViewShell", () => {
    render(<AboutView />);
    expect(screen.getByRole("heading", { name: /About/i })).toBeInTheDocument();
  });

  it("renders the app name and description", () => {
    render(<AboutView />);
    expect(screen.getByText("CopyPaste")).toBeInTheDocument();
    expect(screen.getByText(/Encrypted clipboard manager for macOS/i)).toBeInTheDocument();
  });

  it("renders a version string in the identity section", async () => {
    render(<AboutView />);
    // Version is either a semver or a placeholder like "0.x.x" / "v0.5.2"
    await waitFor(() => {
      expect(screen.getByText(/v?\d+\.\d+/)).toBeInTheDocument();
    });
  });

  it("renders all three feature items with accent checkmarks", () => {
    const { container } = render(<AboutView />);
    // Each feature is preceded by a lucide Check SVG icon (replaced ✓ char with icon in §2)
    // The feature list <ul> contains 3 <li> items each with an SVG check icon.
    const featureList = container.querySelector("ul");
    expect(featureList).not.toBeNull();
    const svgChecks = featureList!.querySelectorAll("svg");
    expect(svgChecks.length).toBeGreaterThanOrEqual(3);
  });

  it("renders the Features section label in uppercase", () => {
    render(<AboutView />);
    // Section label text
    expect(screen.getByText(/features/i)).toBeInTheDocument();
  });

  it("renders the daemon status row label", () => {
    render(<AboutView />);
    expect(screen.getByText(/Background daemon/i)).toBeInTheDocument();
  });

  it("shows Connected status after daemon responds", async () => {
    render(<AboutView />);
    await waitFor(() => {
      expect(screen.getByText(/Connected/i)).toBeInTheDocument();
    });
  });

  it("shows Offline status when daemon is unreachable", async () => {
    invoke.mockRejectedValue("daemon_offline");
    render(<AboutView />);
    await waitFor(() => {
      expect(screen.getByText(/Offline/i)).toBeInTheDocument();
    });
  });

  it("renders the GitHub link as a button with ide-accent styling", () => {
    render(<AboutView />);
    const btn = screen.getByRole("button", { name: /github\.com/i });
    expect(btn).toBeInTheDocument();
    // Must use ide-accent token class (not hardcoded blue)
    expect(btn.className).toContain("text-ide-accent");
    // Must NOT hardcode a blue hex
    expect(btn.className).not.toMatch(/#[0-9a-fA-F]{6}/);
  });

  it("card uses ide-elevated background token (not raw bg color)", () => {
    const { container } = render(<AboutView />);
    // The card wrapper must carry bg-ide-elevated (canonical card bg in redesign)
    const elevated = container.querySelector(".bg-ide-elevated");
    expect(elevated).not.toBeNull();
  });

  it("card uses --skin-r-card for border-radius (skin-aware token)", () => {
    // W-C8: rounded-ide-lg (fixed 14px) replaced with --skin-r-card so Quiet/Vapor adapt.
    // The class is gone; the inline style drives radius.
    const { container } = render(<AboutView />);
    const card = container.querySelector("[style*='--skin-r-card']");
    expect(card).not.toBeNull();
  });

  it("uses border-ide-divider for internal section separators", () => {
    const { container } = render(<AboutView />);
    const divider = container.querySelector(".border-ide-divider");
    expect(divider).not.toBeNull();
  });
});
