import { describe, expect, it } from "vitest";

import { syncReadinessMessage, syncReadinessOf, syncReadinessRecovery, type SyncReadinessInput } from "./syncReadiness";

const READY: SyncReadinessInput = {
    service: { data: true, isPending: false, isError: false },
    config: { data: { config: { sync_enabled: true } }, isPending: false, isError: false },
    peers: { data: [{}], isPending: false, isError: false },
};

describe("sync readiness", () => {
    it.each([
        ["ready", {}, null],
        ["service-loading", { service: { data: undefined, isPending: true, isError: false } }, null],
        ["service-unavailable", { service: { data: true, isPending: false, isError: true } }, "retry-service"],
        ["config-loading", { config: { data: undefined, isPending: true, isError: false } }, null],
        ["config-unavailable", { config: { data: READY.config.data, isPending: false, isError: true } }, "retry-config"],
        ["disabled", { config: { data: { config: { sync_enabled: false } }, isPending: false, isError: false } }, "enable-sync"],
        ["peers-loading", { peers: { data: undefined, isPending: true, isError: false } }, null],
        ["peers-unavailable", { peers: { data: READY.peers.data, isPending: false, isError: true } }, "retry-peers"],
        ["no-peers", { peers: { data: [], isPending: false, isError: false } }, null],
    ] as const)("classifies %s", (expected, changed, recovery) => {
        expect(syncReadinessOf({ ...READY, ...changed })).toBe(expected);
        expect(syncReadinessRecovery(expected)).toBe(recovery);
        if (expected !== "ready") expect(syncReadinessMessage(expected).length).toBeGreaterThan(0);
    });

    it("puts service reachability before downstream failures", () => {
        expect(syncReadinessOf({
            service: { data: undefined, isPending: false, isError: true },
            config: { data: undefined, isPending: false, isError: true },
            peers: { data: undefined, isPending: false, isError: true },
        })).toBe("service-unavailable");
    });

    it("keeps a known disabled master ahead of a pending peer read", () => {
        expect(syncReadinessOf({
            ...READY,
            config: { data: { config: { sync_enabled: false } }, isPending: false, isError: false },
            peers: { data: undefined, isPending: true, isError: false },
        })).toBe("disabled");
    });
});
