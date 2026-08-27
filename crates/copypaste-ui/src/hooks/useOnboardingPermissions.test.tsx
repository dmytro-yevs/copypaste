import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useOnboardingPermissions } from "./useOnboardingPermissions";

const mocks = vi.hoisted(() => ({
  permissionSnapshot: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
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

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
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
      <span>{permissions.data?.notifications.status ?? "no-snapshot"}</span>
      <button type="button" onClick={() => void permissions.refetch()}>
        Refresh
      </button>
    </main>
  );
}

beforeEach(() => {
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
});
