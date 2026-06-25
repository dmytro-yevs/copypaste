/**
 * CopyPaste-1jms.32: transport chip must distinguish P2P / Relay / Supabase.
 *
 * When peer.transport is set by the daemon the chip shows the 3-way label;
 * when absent (older daemon or no transport active) the UI falls back to the
 * local_ip/address heuristic (P2P vs Cloud).
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { PeerRow } from "./DeviceCard";
import type { PairedDevice } from "../lib/ipc";

const BASE_PEER: PairedDevice = {
  fingerprint: "aabbccdd11223344aabbccdd11223344aabbccdd11223344aabbccdd11223344",
  name: "Test Device",
  added_at: 1700000000,
  address: null,
  sync_key_b64: null,
  model: "iPhone 15",
  os_version: "iOS 17",
  app_version: "0.7.4",
  local_ip: null,
  public_ip: null,
  first_sync_at: 1700000100,
  last_sync_at: 1700000200,
  online: true,
  last_seen_secs: 5,
  latency_ms: 12,
  trust: "verified",
};

const NOOP = vi.fn();

describe("CopyPaste-1jms.32: transport chip renders the correct label", () => {
  it("shows 'P2P' chip when peer.transport is 'p2p'", () => {
    const peer: PairedDevice = { ...BASE_PEER, transport: "p2p" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );
    expect(screen.getByText("P2P")).toBeTruthy();
    expect(screen.queryByText("Relay")).toBeNull();
    expect(screen.queryByText("Supabase")).toBeNull();
    expect(screen.queryByText("Cloud")).toBeNull();
  });

  it("shows 'Relay' chip when peer.transport is 'relay'", () => {
    const peer: PairedDevice = { ...BASE_PEER, transport: "relay" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={false}
      />
    );
    expect(screen.getByText("Relay")).toBeTruthy();
    expect(screen.queryByText("P2P")).toBeNull();
    expect(screen.queryByText("Supabase")).toBeNull();
    expect(screen.queryByText("Cloud")).toBeNull();
  });

  it("shows 'Supabase' chip when peer.transport is 'supabase'", () => {
    const peer: PairedDevice = { ...BASE_PEER, transport: "supabase" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={false}
      />
    );
    expect(screen.getByText("Supabase")).toBeTruthy();
    expect(screen.queryByText("P2P")).toBeNull();
    expect(screen.queryByText("Relay")).toBeNull();
    expect(screen.queryByText("Cloud")).toBeNull();
  });

  it("falls back to 'P2P' heuristic when transport is absent and local_ip is set", () => {
    // transport absent (older daemon) but local_ip present → heuristic says P2P
    const peer: PairedDevice = {
      ...BASE_PEER,
      local_ip: "192.168.1.5",
      transport: undefined,
    };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );
    expect(screen.getByText("P2P")).toBeTruthy();
    expect(screen.queryByText("Cloud")).toBeNull();
    expect(screen.queryByText("Relay")).toBeNull();
    expect(screen.queryByText("Supabase")).toBeNull();
  });

  it("falls back to 'Cloud' heuristic when transport is null and no local_ip/address", () => {
    // transport: null (daemon knows there's no transport) AND no local_ip/address
    const peer: PairedDevice = {
      ...BASE_PEER,
      local_ip: null,
      address: null,
      transport: null,
    };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={100}
        liveOnline={false}
      />
    );
    expect(screen.getByText("Cloud")).toBeTruthy();
    expect(screen.queryByText("P2P")).toBeNull();
    expect(screen.queryByText("Relay")).toBeNull();
    expect(screen.queryByText("Supabase")).toBeNull();
  });

  it("Relay chip uses warning color token (not sky or accent)", () => {
    const peer: PairedDevice = { ...BASE_PEER, transport: "relay" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={false}
      />
    );
    const chip = screen.getByText("Relay");
    expect(chip.className).toMatch(/ide-warning/);
    expect(chip.className).not.toMatch(/ide-sky/);
    expect(chip.className).not.toMatch(/ide-accent/);
  });

  it("P2P chip uses sky color token", () => {
    const peer: PairedDevice = { ...BASE_PEER, transport: "p2p" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );
    const chip = screen.getByText("P2P");
    expect(chip.className).toMatch(/ide-sky/);
    expect(chip.className).not.toMatch(/ide-warning/);
  });

  it("Supabase chip uses accent color token", () => {
    const peer: PairedDevice = { ...BASE_PEER, transport: "supabase" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={false}
      />
    );
    const chip = screen.getByText("Supabase");
    expect(chip.className).toMatch(/ide-accent/);
    expect(chip.className).not.toMatch(/ide-sky/);
  });
});
