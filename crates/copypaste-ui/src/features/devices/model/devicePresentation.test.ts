import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import type { PeerInfo } from "@/lib/ipc";
import {
    connectionSummary,
    deviceIconKind,
    peerIdentity,
    peerStatus,
} from "./devicePresentation";
import { cloudConnectionPresentation } from "./cloud";
import {
    peerPresentationState,
    peerPresenceLabel,
    peerRowStatus,
} from "./status";
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

function peerWithPresence(
    state: "online" | "offline" | "unknown" | undefined,
    lastSeenMs: number,
): PeerInfo {
    const now = Date.now();
    return {
        ...PEER,
        last_seen_ms: lastSeenMs,
        details: state === undefined
            ? undefined
            : {
                  profile: null,
                  endpoint: null,
                  latency: null,
                  presence: {
                      state,
                      last_seen_ms: lastSeenMs,
                      provenance: "observed",
                      trust: "local",
                      observed_at_ms: now,
                      fresh_until_ms: now + 60_000,
                  },
                  public_ip: { availability: "unavailable" },
                  geo: { availability: "unavailable" },
              },
    };
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

    it("uses only a peer's generated profile for its identity", () => {
        expect(peerIdentity({ ...PEER, name: "Windows phone" })).toEqual({
            platform: "unknown",
            formFactor: "unknown",
            source: "unknown",
        });
        expect(
            peerIdentity({
                ...PEER,
                details: {
                    profile: {
                        display_name: "Whatever the peer called itself",
                        app_version: null,
                        protocol_version: null,
                        platform: "windows",
                        device_class: "laptop",
                        os_name: null,
                        os_version: null,
                        model: null,
                        provenance: "self_reported",
                        trust: "unverified",
                        observed_at_ms: 1,
                        fresh_until_ms: null,
                    },
                    endpoint: null,
                    latency: null,
                    presence: null,
                    public_ip: { availability: "unavailable" },
                    geo: { availability: "unavailable" },
                },
            }),
        ).toEqual({
            platform: "windows",
            formFactor: "laptop",
            source: "peer-asserted",
        });
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
            a11y: {},
        });
    });

    it("resolves peer presentation precedence consistently for cards and rows", () => {
        const now = Date.now();
        const failure = (kind: "auth_failed" | "peer_failed" | "pairing_code") => ({
            failure: {
                at: now,
                kind,
                retryable: false,
                durationMs: null,
            },
        });
        const cases = [
            {
                peer: peerWithPresence(undefined, now - 1),
                health: failure("auth_failed"),
                state: "failing",
                label: "Sync failed",
            },
            {
                peer: peerWithPresence(undefined, now - 1),
                health: failure("peer_failed"),
                state: "failing",
                label: "Sync failed",
            },
            {
                peer: peerWithPresence(undefined, now - 1),
                health: failure("pairing_code"),
                state: "failing",
                label: "Sync failed",
            },
            {
                peer: peerWithPresence(undefined, 0),
                health: undefined,
                state: "waiting",
                label: "Waiting",
            },
            {
                peer: peerWithPresence(undefined, now - STALE_AFTER_MS - 1),
                health: undefined,
                state: "presence-unknown",
                label: "Presence unknown",
            },
            {
                peer: peerWithPresence("online", now - STALE_AFTER_MS - 1),
                health: undefined,
                state: "stalled",
                label: "Needs attention",
            },
            {
                peer: peerWithPresence("offline", now - 1),
                health: undefined,
                state: "away",
                label: "Away",
            },
        ] as const;

        for (const expected of cases) {
            const state = peerPresentationState(
                expected.peer,
                expected.health,
                false,
            );
            expect(state).toBe(expected.state);
            expect(peerStatus(expected.peer, expected.health, false).label).toBe(
                expected.label,
            );
            expect(peerRowStatus(state).label).toBeTruthy();
        }
    });

    it("owns cloud icon, detail, action, and a11y facts in the descriptor", () => {
        expect(cloudConnectionPresentation(undefined, true, false)).toEqual({
            state: "unavailable",
            icon: "cloudOff",
            title: "Encrypted cloud",
            detail: "Cloud status is unavailable.",
            busy: false,
            role: "status",
            live: "polite",
            action: { label: "Manage", icon: "settings" },
        });
    });

    it("keeps canonical device vocabulary out of component-local maps", () => {
        const sourceRoot = resolve(import.meta.dirname, "..");
        for (const file of [
            "components/DeviceStatus.tsx",
            "components/PeerRow.tsx",
            "components/CloudConnectionCard.tsx",
            "patterns/DiscoveryStage.tsx",
        ]) {
            const source = readFileSync(resolve(sourceRoot, file), "utf8");
            expect(source).not.toMatch(/STATUS_ICON|const BADGE|cloudPresentation|const copy/);
        }
    });
});
