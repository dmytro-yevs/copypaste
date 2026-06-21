/**
 * Tests for SCRD-1, SCRD-2, SCRD-3 / SYNC-5 (bd CopyPaste-5917.11, .8, .5, 1jms.26)
 *
 * SCRD-3 / SYNC-5: stale peer presence expiry
 *   - A peer whose last `connected` event is older than PEER_PRESENCE_TTL_MS
 *     must be reported as offline by `expireStale()`.
 *   - `resetAllOffline()` immediately flips all online peers to offline.
 *
 * SCRD-2: duplicate last-sync time in PeerRow
 *   - The "Synced X" paragraph below the metadata grid must be absent; only
 *     the MetaRow "Last sync" entry inside the grid should appear.
 *
 * SCRD-1: online status-dot glow
 *   - The glow `boxShadow` uses `var(--success)` which is globally aliased to
 *     `var(--ide-success)` in index.css — rendering is correct. This test
 *     verifies the inline style is present on the dot when online=true and
 *     absent when online=false.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { PeerRow, StatusDot } from "./DeviceCard";
import { usePeerPresence, PEER_PRESENCE_TTL_MS } from "../lib/peerPresence";
import type { PairedDevice } from "../lib/ipc";

// ---------------------------------------------------------------------------
// Shared fixture
// ---------------------------------------------------------------------------

const BASE_PEER: PairedDevice = {
  fingerprint: "aabbccdd11223344aabbccdd11223344aabbccdd11223344aabbccdd11223344",
  name: "Alice's iPhone",
  added_at: 1700000000,
  address: "192.168.1.5:4242",
  sync_key_b64: null,
  model: "iPhone 15",
  os_version: "iOS 17",
  app_version: "0.7.4",
  local_ip: "192.168.1.5",
  public_ip: null,
  first_sync_at: 1700000100,
  last_sync_at: 1700000200,
  online: true,
  last_seen_secs: 5,
  latency_ms: 12,
};

const NOOP = vi.fn();

// ---------------------------------------------------------------------------
// SCRD-3 / SYNC-5: stale peer presence expiry (peerPresence store)
// ---------------------------------------------------------------------------

describe("peerPresence store — stale expiry (SCRD-3 / SYNC-5)", () => {
  beforeEach(() => {
    // Reset store to a clean state before each test.
    usePeerPresence.setState({ online: {}, seenAt: {} });
  });

  it("expireStale flips a peer offline when its seenAt is older than PEER_PRESENCE_TTL_MS", () => {
    const staleTs = Date.now() - PEER_PRESENCE_TTL_MS - 1000; // clearly expired
    usePeerPresence.setState({
      online: { "peer-fp-1": true },
      seenAt: { "peer-fp-1": staleTs },
    });

    usePeerPresence.getState().expireStale();

    expect(usePeerPresence.getState().online["peer-fp-1"]).toBe(false);
  });

  it("expireStale keeps a fresh peer online when seenAt is within TTL", () => {
    const freshTs = Date.now() - 1000; // 1 s ago — well within TTL
    usePeerPresence.setState({
      online: { "peer-fp-2": true },
      seenAt: { "peer-fp-2": freshTs },
    });

    usePeerPresence.getState().expireStale();

    expect(usePeerPresence.getState().online["peer-fp-2"]).toBe(true);
  });

  it("expireStale does not touch already-offline peers", () => {
    const staleTs = Date.now() - PEER_PRESENCE_TTL_MS - 5000;
    usePeerPresence.setState({
      online: { "peer-fp-3": false },
      seenAt: { "peer-fp-3": staleTs },
    });

    usePeerPresence.getState().expireStale();

    // Must remain false (no spurious flip)
    expect(usePeerPresence.getState().online["peer-fp-3"]).toBe(false);
  });

  it("resetAllOffline flips all online peers to offline immediately", () => {
    usePeerPresence.setState({
      online: { "peer-a": true, "peer-b": true, "peer-c": false },
      seenAt: { "peer-a": Date.now(), "peer-b": Date.now() },
    });

    usePeerPresence.getState().resetAllOffline();

    const { online } = usePeerPresence.getState();
    expect(online["peer-a"]).toBe(false);
    expect(online["peer-b"]).toBe(false);
    expect(online["peer-c"]).toBe(false);
  });

  it("applyEvents updates seenAt for connected events", () => {
    const before = Date.now();
    usePeerPresence.setState({ online: {}, seenAt: {} });

    usePeerPresence.getState().applyEvents([
      { kind: "connected", fingerprint: "peer-fp-4" },
    ]);

    const after = Date.now();
    const seenAt = usePeerPresence.getState().seenAt["peer-fp-4"] ?? 0;
    expect(seenAt).toBeGreaterThanOrEqual(before);
    expect(seenAt).toBeLessThanOrEqual(after);
  });

  it("a peer receiving a connected event followed by expireStale stays online within TTL", () => {
    usePeerPresence.getState().applyEvents([
      { kind: "connected", fingerprint: "peer-fp-5" },
    ]);

    // Expire immediately — seenAt is just now, so TTL has not elapsed.
    usePeerPresence.getState().expireStale();

    expect(usePeerPresence.getState().online["peer-fp-5"]).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// SCRD-2: duplicate last-sync time in PeerRow
// ---------------------------------------------------------------------------

describe("PeerRow — no duplicate last-sync time (SCRD-2)", () => {
  it("renders 'Last sync' label exactly once (in the metadata grid, not again below)", () => {
    const { container } = render(
      <PeerRow
        peer={BASE_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    // "Last sync" label should appear exactly once (inside the MetaRow grid).
    const occurrences = Array.from(container.querySelectorAll("span")).filter(
      (el) => el.textContent?.trim() === "Last sync"
    );
    expect(occurrences).toHaveLength(1);
  });

  it("does not render a standalone 'Synced …' paragraph below the grid", () => {
    const { container } = render(
      <PeerRow
        peer={BASE_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    // The duplicate paragraph had the pattern "Synced <timestamp>" as a <p> element.
    // After the fix, no <p> inside PeerRow should start with "Synced ".
    const syncedParas = Array.from(container.querySelectorAll("p")).filter((el) =>
      el.textContent?.startsWith("Synced ")
    );
    expect(syncedParas).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// SCRD-1: online status-dot glow via var(--success)
// ---------------------------------------------------------------------------

describe("StatusDot — glow renders with --success token (SCRD-1)", () => {
  it("online dot has a boxShadow inline style referencing --success", () => {
    const { container } = render(<StatusDot online={true} lastSeenSecs={2} />);

    // The inner dot span (not the pulse ring) carries the boxShadow.
    const dotSpans = Array.from(container.querySelectorAll("span[title='Online']"));
    expect(dotSpans.length).toBeGreaterThan(0);

    const dotEl = dotSpans[0] as HTMLElement;
    // jsdom doesn't resolve CSS vars but the raw style attribute must
    // reference --success so the browser can resolve it.
    expect(dotEl.style.boxShadow).toContain("--success");
  });

  it("offline dot has no boxShadow glow", () => {
    const { container } = render(<StatusDot online={false} lastSeenSecs={300} />);

    const dotSpans = Array.from(container.querySelectorAll("span")).filter(
      (el) => el.getAttribute("title")?.startsWith("Offline")
    );
    expect(dotSpans.length).toBeGreaterThan(0);

    const dotEl = dotSpans[0] as HTMLElement;
    expect(dotEl.style.boxShadow ?? "").toBe("");
  });
});
