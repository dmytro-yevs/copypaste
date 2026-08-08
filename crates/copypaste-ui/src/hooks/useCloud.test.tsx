import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  CLOUD_STATUS_KEY,
  useCloudSignIn,
  useCloudSignOut,
  useCloudStatus,
} from "@/hooks/useCloud";
import type { CloudStatusData } from "@/lib/ipc";
import { testClient } from "@/test/harness";

const getCloudStatus = vi.fn();
const cloudSignIn = vi.fn();
const cloudSignOut = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getCloudStatus: () => getCloudStatus(),
    cloudSignIn: (credentials: unknown) => cloudSignIn(credentials),
    cloudSignOut: () => cloudSignOut(),
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
