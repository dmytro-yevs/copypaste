import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { toast } from "sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  CLOUD_STATUS_KEY,
  useCloudSignIn,
  useCloudSignOut,
  useCloudStatus,
  useCloudSyncNow,
} from "@/hooks/useCloud";
import type { CloudStatusData, CloudSyncData } from "@/lib/ipc";
import { testClient } from "@/test/harness";

const getCloudStatus = vi.fn();
const cloudSignIn = vi.fn();
const cloudSignOut = vi.fn();
const syncCloudNow = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getCloudStatus: () => getCloudStatus(),
    cloudSignIn: (credentials: unknown) => cloudSignIn(credentials),
    cloudSignOut: () => cloudSignOut(),
    syncCloudNow: () => syncCloudNow(),
  };
});

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
  },
}));

function cloudStatus(overrides: Partial<CloudStatusData> = {}): CloudStatusData {
  return {
    configured: true,
    signed_in: false,
    key_ready: false,
    email: null,
    last_sync_ms: null,
    last_error: null,
    poll_interval_secs: 60,
    ...overrides,
  };
}

function cloudSync(overrides: Partial<CloudSyncData> = {}): CloudSyncData {
  return {
    uploaded: 2,
    tombstoned: 0,
    downloaded: 3,
    applied: 3,
    skipped_sensitive: 0,
    skipped_undecryptable: 0,
    skipped_forged: 0,
    skipped_future: 0,
    skipped_too_large: 0,
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function wrapper(client: ReturnType<typeof testClient>) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

beforeEach(() => {
  getCloudStatus.mockReset();
  cloudSignIn.mockReset();
  cloudSignOut.mockReset();
  syncCloudNow.mockReset();
  vi.mocked(toast.success).mockReset();
  vi.mocked(toast.warning).mockReset();
});

describe("cloud sync completion toast", () => {
  it("keeps a skipped-row warning available for accessibility scans", async () => {
    syncCloudNow.mockResolvedValue(cloudSync({
      skipped_sensitive: 1,
      skipped_undecryptable: 2,
    }));
    const client = testClient();
    const { result } = renderHook(() => useCloudSyncNow(), {
      wrapper: wrapper(client),
    });

    await act(async () => {
      await result.current.mutateAsync();
    });

    expect(toast.warning).toHaveBeenCalledWith(
      "Cloud sync finished: 2 uploaded, 3 downloaded, 3 skipped",
      { duration: 12_000 },
    );
    expect(toast.success).not.toHaveBeenCalled();
  });

  it("keeps zero-skip completion on the normal success toast", async () => {
    syncCloudNow.mockResolvedValue(cloudSync());
    const client = testClient();
    const { result } = renderHook(() => useCloudSyncNow(), {
      wrapper: wrapper(client),
    });

    await act(async () => {
      await result.current.mutateAsync();
    });

    expect(toast.success).toHaveBeenCalledWith(
      "Cloud sync finished: 2 uploaded, 3 downloaded",
    );
    expect(toast.warning).not.toHaveBeenCalled();
  });
});

describe("cloud status cache mutation ordering", () => {
  it("keeps a completed sign-in newer than an older status response", async () => {
    const signedOut = cloudStatus();
    const signedIn = cloudStatus({
      signed_in: true,
      key_ready: true,
      email: "me@example.com",
    });
    const oldStatus = deferred<CloudStatusData>();
    const client = testClient();
    client.setQueryData(CLOUD_STATUS_KEY, signedOut);
    getCloudStatus.mockReturnValue(oldStatus.promise);
    cloudSignIn.mockResolvedValue(signedIn);

    const { result } = renderHook(
      () => ({ status: useCloudStatus(), signIn: useCloudSignIn() }),
      { wrapper: wrapper(client) },
    );
    await waitFor(() => expect(getCloudStatus).toHaveBeenCalledTimes(1));

    await act(async () => {
      await result.current.signIn.mutateAsync({
        email: "me@example.com",
        password: "password",
        passphrase: "passphrase",
      });
    });
    oldStatus.resolve(signedOut);
    await act(async () => {
      await oldStatus.promise;
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(client.getQueryData(CLOUD_STATUS_KEY)).toEqual(signedIn);
      expect(result.current.status.data).toEqual(signedIn);
    });
  });

  it("keeps a completed sign-out newer than an older status response", async () => {
    const signedIn = cloudStatus({
      signed_in: true,
      key_ready: true,
      email: "me@example.com",
    });
    const signedOut = cloudStatus();
    const oldStatus = deferred<CloudStatusData>();
    const client = testClient();
    client.setQueryData(CLOUD_STATUS_KEY, signedIn);
    getCloudStatus.mockReturnValue(oldStatus.promise);
    cloudSignOut.mockResolvedValue(signedOut);

    const { result } = renderHook(
      () => ({ status: useCloudStatus(), signOut: useCloudSignOut() }),
      { wrapper: wrapper(client) },
    );
    await waitFor(() => expect(getCloudStatus).toHaveBeenCalledTimes(1));

    await act(async () => {
      await result.current.signOut.mutateAsync();
    });
    oldStatus.resolve(signedIn);
    await act(async () => {
      await oldStatus.promise;
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(client.getQueryData(CLOUD_STATUS_KEY)).toEqual(signedOut);
      expect(result.current.status.data).toEqual(signedOut);
    });
  });
});
