import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import type { PeerInfo } from "@/lib/ipc";
import {
    connectionSummary,
    deviceIconKind,
    peerStatus,
} from "./devicePresentation";
import { cloudConnectionPresentation } from "./cloud";
import { peerPresenceLabel } from "./status";
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
            status: "info",
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
            status: "attention",
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
            status: "attention",
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
            status: "attention",
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
        ["desktop", "monitor"],
        ["laptop", "laptop"],
        ["phone", "mobile"],
        ["tablet", "tablet"],
        ["unknown", "devices"],
    ] as const)(
        "maps every generated DeviceClass %s to the canonical %s icon",
        (formFactor, expected) => {
            expect(
                deviceIconKind({
                    platform: "android",
                    formFactor,
                    source: "peer-asserted",
                }),
            ).toBe(expected);
        },
    );

    it("keeps an unknown class generic regardless of its reported platform", () => {
        for (const platform of ["android", "macos", "windows", "unknown"] as const) {
            expect(deviceIconKind({ platform, formFactor: "unknown", source: "peer-asserted" })).toBe("devices");
        }
    });
});

describe("device status descriptors", () => {
    it.each([
        ["online", "On this network"],
        ["offline", "Not seen on this network"],
        ["unknown", "Network presence unknown"],
    ] as const)("maps generated presence %s to %s", (presence, label) => {
        expect(peerPresenceLabel(presence)).toBe(label);
    });

    it("owns peer icon, tone, busy, and live facts in the descriptor", () => {
        expect(peerStatus(PEER, undefined, true)).toMatchObject({
            icon: "refresh",
            label: "Syncing",
            tone: "busy",
            busy: true,
            live: "off",
        });
    });

    it("owns cloud icon, detail, action, and a11y facts in the descriptor", () => {
        expect(cloudConnectionPresentation(undefined, true, false)).toEqual({
            state: "unavailable",
            icon: "cloudOff",
            title: "Encrypted cloud",
            detail: "Cloud status is unavailable.",
            busy: false,
            live: "polite",
            action: { label: "Manage", icon: "settings" },
        });
    });

    it("keeps status and cloud vocabulary out of component-local maps", () => {
        const sourceRoot = resolve(import.meta.dirname, "..");
        for (const file of ["components/DeviceStatus.tsx", "components/PeerRow.tsx", "components/CloudConnectionCard.tsx"]) {
            const source = readFileSync(resolve(sourceRoot, file), "utf8");
            expect(source).not.toMatch(/STATUS_ICON|const BADGE|cloudPresentation/);
        }
    });
});
