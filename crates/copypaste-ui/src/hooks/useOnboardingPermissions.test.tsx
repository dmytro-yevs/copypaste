import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  ONBOARDING_PERMISSIONS_KEY,
  useOnboardingPermissions,
  usePermissionRequest,
} from "./useOnboardingPermissions";

const mocks = vi.hoisted(() => ({
  permissionOpenSettings: vi.fn(),
  permissionRequest: vi.fn(),
  permissionSnapshot: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  permissionOpenSettings: mocks.permissionOpenSettings,
  permissionRequest: mocks.permissionRequest,
  permissionSnapshot: mocks.permissionSnapshot,
}));

vi.mock("@/lib/ipcCall", () => ({
  hasNativeBridge: () => true,
}));

vi.mock("@/lib/platform", () => ({
  isAndroidPlatform: () => true,
}));

const granted = {
  platform: "android",
  notifications: { id: "notifications", status: "granted", required: false },
  tile: { id: "tile", status: "not_required", required: false },
  clipboardStatus: "not_required",
} as const;

const prompt = {
  ...granted,
  notifications: { ...granted.notifications, status: "prompt" },
} as const;

interface Deferred<T> {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((onResolve) => {
    resolve = onResolve;
  });
  return { promise, resolve };
}

let client: QueryClient;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

function makeClient(): QueryClient {
  return new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
}

function StartupProbe() {
  const permissions = useOnboardingPermissions();
  const state = permissions.isPending
    ? "pending"
    : permissions.error !== null
      ? "error"
      : "ready";
  return (
    <main>
      <h1>App shell</h1>
      <output>{state}</output>
      <span>{permissions.isFetching ? "fetching" : "idle"}</span>
      <span>{permissions.data?.notifications.status ?? "no-snapshot"}</span>
      <button type="button" onClick={() => void permissions.refetch()}>
        Refresh
      </button>
    </main>
  );
}

function MutationRaceProbe() {
  const permissions = useOnboardingPermissions();
  const request = usePermissionRequest();
  return (
    <main>
      <output>{permissions.data?.notifications.status ?? "no-snapshot"}</output>
      <span>{permissions.isFetching ? "fetching" : "idle"}</span>
      <button type="button" onClick={() => request.mutate("notifications")}>
        Request permission
      </button>
      <button type="button" onClick={() => void permissions.refetch()}>
        Refresh
      </button>
    </main>
  );
}

beforeEach(() => {
  client = makeClient();
  mocks.permissionOpenSettings.mockReset();
  mocks.permissionRequest.mockReset();
  mocks.permissionSnapshot.mockReset();
});

describe("onboarding permission hydration", () => {
  it("renders the app shell while the native snapshot remains pending", () => {
    mocks.permissionSnapshot.mockReturnValue(new Promise(() => {}));

    render(<StartupProbe />, { wrapper });

    expect(screen.getByRole("heading", { name: "App shell" })).toBeTruthy();
    expect(screen.getByText("pending")).toBeTruthy();
    expect(screen.getByText("no-snapshot")).toBeTruthy();
  });

  it("aborts an unmounted snapshot and ignores its late result", async () => {
    const late = deferred<typeof granted>();
    let signal: AbortSignal | undefined;
    mocks.permissionSnapshot.mockImplementation((options) => {
      signal = options.signal;
      return late.promise;
    });

    const mounted = render(<StartupProbe />, { wrapper });
    await waitFor(() => expect(mocks.permissionSnapshot).toHaveBeenCalledOnce());
    expect(signal?.aborted).toBe(false);

    mounted.unmount();
    expect(signal?.aborted).toBe(true);
    late.resolve(granted);
    await Promise.resolve();
    await Promise.resolve();

    expect(client.getQueryData(ONBOARDING_PERMISSIONS_KEY)).toBeUndefined();
  });

  it("recovers from a failed snapshot on a successful refresh", async () => {
    const user = userEvent.setup();
    mocks.permissionSnapshot
      .mockRejectedValueOnce(new Error("permission host unavailable"))
      .mockResolvedValueOnce(granted);

    render(<StartupProbe />, { wrapper });
    await waitFor(() => expect(screen.getByText("error")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => expect(screen.getByText("ready")).toBeTruthy());
    expect(screen.getByText("granted")).toBeTruthy();
  });

  it("keeps a failed refresh explicit instead of replacing it with permission truth", async () => {
    const user = userEvent.setup();
    mocks.permissionSnapshot
      .mockResolvedValueOnce(granted)
      .mockRejectedValueOnce(new Error("permission host unavailable"));

    render(<StartupProbe />, { wrapper });
    await waitFor(() => expect(screen.getByText("ready")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => expect(screen.getByText("error")).toBeTruthy());
    expect(screen.getByText("granted")).toBeTruthy();
  });

  it("fences a refresh started while a permission mutation is pending", async () => {
    const user = userEvent.setup();
    const lateSnapshot = deferred<typeof prompt>();
    const mutation = deferred<typeof granted>();
    let refreshSignal: AbortSignal | undefined;
    mocks.permissionSnapshot
      .mockResolvedValueOnce(prompt)
      .mockImplementationOnce((options) => {
        refreshSignal = options.signal;
        return lateSnapshot.promise;
      });
    mocks.permissionRequest.mockReturnValue(mutation.promise);

    render(<MutationRaceProbe />, { wrapper });
    await waitFor(() => expect(screen.getByText("prompt")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "Request permission" }));
    await waitFor(() => expect(mocks.permissionRequest).toHaveBeenCalledOnce());
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(screen.getByText("fetching")).toBeTruthy());

    mutation.resolve(granted);
    await waitFor(() => expect(screen.getByText("granted")).toBeTruthy());
    expect(refreshSignal?.aborted).toBe(true);
    lateSnapshot.resolve(prompt);
    await Promise.resolve();
    await Promise.resolve();

    expect(screen.getByText("granted")).toBeTruthy();
    expect(client.getQueryData(ONBOARDING_PERMISSIONS_KEY)).toEqual(granted);
  });

  it("aborts a stale refresh when a permission mutation starts", async () => {
    const user = userEvent.setup();
    const lateSnapshot = deferred<typeof prompt>();
    const mutation = deferred<typeof granted>();
    let refreshSignal: AbortSignal | undefined;
    mocks.permissionSnapshot
      .mockResolvedValueOnce(prompt)
      .mockImplementationOnce((options) => {
        refreshSignal = options.signal;
        return lateSnapshot.promise;
      });
    mocks.permissionRequest.mockReturnValue(mutation.promise);

    render(<MutationRaceProbe />, { wrapper });
    await waitFor(() => expect(screen.getByText("prompt")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(screen.getByText("fetching")).toBeTruthy());
    expect(refreshSignal?.aborted).toBe(false);

    await user.click(screen.getByRole("button", { name: "Request permission" }));
    await waitFor(() => expect(mocks.permissionRequest).toHaveBeenCalledOnce());
    expect(refreshSignal?.aborted).toBe(true);
    mutation.resolve(granted);
    await waitFor(() => expect(screen.getByText("granted")).toBeTruthy());

    lateSnapshot.resolve(prompt);
    await Promise.resolve();
    await Promise.resolve();

    expect(screen.getByText("granted")).toBeTruthy();
    expect(client.getQueryData(ONBOARDING_PERMISSIONS_KEY)).toEqual(granted);
  });
});
