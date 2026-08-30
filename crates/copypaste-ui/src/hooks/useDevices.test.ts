import { createElement, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SyncResult } from "@/lib/ipc";
import { useSyncNow } from "./useDevices";

const ipc = vi.hoisted(() => ({ syncNow: vi.fn() }));
const notifications = vi.hoisted(() => ({
  info: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  syncNow: (pairingId?: string) => ipc.syncNow(pairingId),
}));

vi.mock("sonner", () => ({
  toast: Object.assign(notifications.info, {
    success: notifications.success,
    warning: notifications.warning,
    error: notifications.error,
  }),
}));

function syncResult(over: Partial<SyncResult> = {}): SyncResult {
  return {
    pairing_id: "peer-1",
    name: "Phone",
    sent: 3,
    received: 2,
    duration_ms: 25,
    error: null,
    skipped_too_large: 0,
    ...over,
  };
}

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return createElement(QueryClientProvider, { client }, children);
}

async function sync(results: SyncResult[]) {
  ipc.syncNow.mockResolvedValueOnce(results);
  const hook = renderHook(() => useSyncNow(), { wrapper });
  await act(async () => {
    await hook.result.current.mutateAsync(undefined);
  });
}

beforeEach(() => {
  ipc.syncNow.mockReset();
  for (const notification of Object.values(notifications)) notification.mockReset();
});

describe("useSyncNow size-refusal feedback", () => {
  it("keeps the no-peer branch", async () => {
    await sync([]);
    expect(notifications.info).toHaveBeenCalledWith(
      "Nothing to sync — no paired devices",
    );
  });

  it("shows success only when every completed peer explicitly reports zero", async () => {
    await sync([syncResult(), syncResult({ pairing_id: "peer-2" })]);
    expect(notifications.success).toHaveBeenCalledWith(
      "Synced 2 devices — sent 6, received 4",
    );
  });

  it("warns about a known positive size-refusal count", async () => {
    await sync([syncResult({ skipped_too_large: 2 })]);
    expect(notifications.warning).toHaveBeenCalledWith(
      "Synced 1 device — sent 3, received 2; oversized-item refusals: 2",
    );
  });

  it("sums peer-session refusals without treating them as unique items", async () => {
    await sync([
      syncResult({ skipped_too_large: 1 }),
      syncResult({ pairing_id: "peer-2", skipped_too_large: 1 }),
    ]);
    expect(notifications.warning).toHaveBeenCalledWith(
      "Synced 2 devices — sent 6, received 4; oversized-item refusals: 2",
    );
  });

  it("warns when a completed peer did not report a size-refusal count", async () => {
    await sync([syncResult({ skipped_too_large: undefined })]);
    expect(notifications.warning).toHaveBeenCalledWith(
      "Synced 1 device — sent 3, received 2; oversized-item refusal counts are unavailable for one or more peer syncs",
    );
  });

  it("does not claim a total when known positive and unavailable counts mix", async () => {
    await sync([
      syncResult({ skipped_too_large: 2 }),
      syncResult({ pairing_id: "peer-2", skipped_too_large: undefined }),
    ]);
    expect(notifications.warning).toHaveBeenCalledWith(
      "Synced 2 devices — sent 6, received 4; oversized-item refusals: 2, and counts are unavailable for one or more peer syncs",
    );
  });

  it("keeps peer failures ahead of size-refusal feedback", async () => {
    await sync([
      syncResult({ skipped_too_large: 2 }),
      syncResult({
        pairing_id: "peer-2",
        error: { code: "peer_unreachable", retryable: true },
        skipped_too_large: undefined,
      }),
    ]);
    expect(notifications.warning).toHaveBeenCalledWith(
      "Synced 1 of 2 devices — 1 failed",
    );
    expect(notifications.warning).toHaveBeenCalledTimes(1);
  });
});
