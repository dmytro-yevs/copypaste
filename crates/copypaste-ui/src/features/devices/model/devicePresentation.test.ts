import { describe, expect, it } from "vitest";

import type { PeerInfo } from "@/lib/ipc";
import {
    DeviceIconKind,
    connectionSummary,
    deviceIconKind,
} from "./devicePresentation";
import { STALE_AFTER_MS } from "./peerState";

const PEER = {
    pairing_id: "peer-1",
    name: "Nearby phone",
    online: true,
    last_addr: "192.0.2.8:47654",
    last_seen_ms: 0,
    last_sync_at: null,
    paired_at: Date.now(),
} as PeerInfo;

function summary(overrides: Partial<Parameters<typeof connectionSummary>[0]> = {}) {
    return connectionSummary({
        serviceOffline: false,
        serviceStarting: false,
        syncing: false,
        peersLoaded: true,
        peersFailed: false,
        peers: [PEER],
        health: {},
        ...overrides,
    });
}

describe("connectionSummary", () => {
    it("hides the healthy success summary", () => {
        expect(summary()).toBeNull();
        expect(summary({ peers: [] })).toBeNull();
    });

    it("shows active sync operations", () => {
        expect(summary({ syncing: true })).toMatchObject({
            title: "Syncing your devices",
            busy: true,
            state: "syncing",
        });
    });

    it("hides offline, stale, and unreachable peers as transient", () => {
        const offlinePeer = {
            ...PEER,
            online: false,
            last_seen_ms: Date.now() - STALE_AFTER_MS - 1,
        };

        expect(summary({ peers: [offlinePeer] })).toBeNull();
        expect(
            summary({
                peers: [offlinePeer],
                health: {
                    [offlinePeer.pairing_id]: {
                        failure: {
                            at: Date.now(),
                            kind: "peer_unreachable",
                            retryable: true,
                            durationMs: null,
                        },
                    },
                },
            }),
        ).toBeNull();
    });

    it("shows a retry only for an explicit failed session on a visible peer", () => {
        expect(
            summary({
                health: {
                    [PEER.pairing_id]: {
                        failure: {
                            at: Date.now(),
                            kind: "peer_failed",
                            retryable: true,
                            durationMs: null,
                        },
                    },
                },
            }),
        ).toMatchObject({
            title: "Sync with Nearby phone failed",
            state: "attention",
            action: {
                kind: "retry-peer",
                label: "Try again",
                pairingId: PEER.pairing_id,
            },
        });
    });

    it("keeps an explicit failed session actionable after the peer goes offline", () => {
        expect(
            summary({
                peers: [{ ...PEER, online: false }],
                health: {
                    [PEER.pairing_id]: {
                        failure: {
                            at: Date.now(),
                            kind: "peer_failed",
                            retryable: true,
                            durationMs: null,
                        },
                    },
                },
            }),
        ).toMatchObject({
            title: "Sync with Nearby phone failed",
            state: "attention",
            action: { kind: "retry-peer" },
        });
    });

    it("shows evidence-backed pairing authentication failures", () => {
        expect(
            summary({
                peers: [{ ...PEER, online: false }],
                health: {
                    [PEER.pairing_id]: {
                        failure: {
                            at: Date.now(),
                            kind: "auth_failed",
                            retryable: false,
                            durationMs: null,
                        },
                    },
                },
            }),
        ).toMatchObject({
            title: "Nearby phone's pairing needs attention",
            state: "attention",
            action: {
                kind: "review-peer",
                label: "Review device",
                pairingId: PEER.pairing_id,
            },
        });
    });
});

describe("device icon presentation", () => {
    it.each([
        ["windows", "desktop", DeviceIconKind.Monitor],
        ["windows", "unknown", DeviceIconKind.Monitor],
        ["macos", "laptop", DeviceIconKind.Laptop],
        ["android", "phone", DeviceIconKind.Mobile],
        ["android", "tablet", DeviceIconKind.Tablet],
        ["android", "unknown", DeviceIconKind.Mobile],
        ["unknown", "unknown", DeviceIconKind.Devices],
    ] as const)(
        "maps %s/%s to the canonical %s icon",
        (platform, formFactor, expected) => {
            expect(
                deviceIconKind({
                    platform,
                    formFactor,
                    source: "peer-asserted",
                }),
            ).toBe(expected);
        },
    );
});
