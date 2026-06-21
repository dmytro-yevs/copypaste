/**
 * DeviceBadge — unit tests (CopyPaste-bdac.31).
 *
 * Verifies: own-device accent variant, remote-device dim variant, null cases,
 * UUID-prefix fallback, and the deviceLabel helper.
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DeviceBadge, deviceLabel } from "./DeviceBadge";

// ---------------------------------------------------------------------------
// deviceLabel helper
// ---------------------------------------------------------------------------

describe("deviceLabel", () => {
  it("returns null when originId is undefined", () => {
    expect(deviceLabel(undefined, "own-123")).toBeNull();
  });

  it("returns null when originId is empty string", () => {
    expect(deviceLabel("", "own-123")).toBeNull();
  });

  it("returns 'This device' when originId matches ownId", () => {
    expect(deviceLabel("abc", "abc")).toBe("This device");
  });

  it("returns the originName when provided for a remote device", () => {
    expect(deviceLabel("remote-id", "own-id", "MacBook Pro")).toBe("MacBook Pro");
  });

  it("falls back to UUID prefix (first 8 chars) when no originName", () => {
    expect(deviceLabel("12345678-abcd-efgh", "own-id")).toBe("12345678");
  });
});

// ---------------------------------------------------------------------------
// DeviceBadge component
// ---------------------------------------------------------------------------

describe("DeviceBadge", () => {
  it("renders nothing when originId is undefined", () => {
    const { container } = render(
      <DeviceBadge originId={undefined} ownId="own-123" />
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing when originId is empty string", () => {
    const { container } = render(
      <DeviceBadge originId="" ownId="own-123" />
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders 'This device' with accent styling for own device", () => {
    render(<DeviceBadge originId="abc" ownId="abc" />);
    const badge = screen.getByText("This device");
    expect(badge).toBeInTheDocument();
    expect(badge.className).toContain("text-ide-accent");
    expect(badge.className).toContain("border-ide-accent/40");
    expect(badge.className).toContain("bg-ide-accent/10");
  });

  it("renders device name with faint styling for remote device", () => {
    render(
      <DeviceBadge originId="remote-id" ownId="own-id" originName="MacBook Pro" />
    );
    const badge = screen.getByText("MacBook Pro");
    expect(badge).toBeInTheDocument();
    expect(badge.className).toContain("text-ide-faint");
    expect(badge.className).not.toContain("text-ide-accent");
  });

  it("renders UUID prefix fallback for unknown remote device", () => {
    render(
      <DeviceBadge originId="12345678-abcd-efgh" ownId="own-id" />
    );
    const badge = screen.getByText("12345678");
    expect(badge).toBeInTheDocument();
    expect(badge.className).toContain("text-ide-faint");
  });

  it("sets title attribute to originId for tooltip", () => {
    render(
      <DeviceBadge originId="remote-uuid-full" ownId="own-id" originName="Studio" />
    );
    const badge = screen.getByTitle("remote-uuid-full");
    expect(badge).toBeInTheDocument();
  });
});
