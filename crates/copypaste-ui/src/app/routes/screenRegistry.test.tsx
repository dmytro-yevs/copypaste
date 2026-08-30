import { Suspense, type ComponentType } from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { Boundary } from "@/app/shell/Boundary";

const routeModules = vi.hoisted(() => ({
  capture: vi.fn(),
  devices: vi.fn(),
  history: vi.fn(),
  settings: vi.fn(),
}));

vi.mock("@/features/capture", () => routeModules.capture());
vi.mock("@/features/devices", () => routeModules.devices());
vi.mock("@/features/history", () => routeModules.history());
vi.mock("@/features/settings", () => routeModules.settings());

type Registry = typeof import("./screenRegistry").screenRegistry;

function component(label: string): ComponentType {
  return () => <div>{label}</div>;
}

async function loadRegistry(): Promise<Registry> {
  vi.resetModules();
  return (await import("./screenRegistry")).screenRegistry;
}

function Route({ registry, view }: { registry: Registry; view: "history" | "devices" }) {
  return (
    <Boundary label={view === "devices" ? "Devices" : "Library"} onReset={registry[view].reset}>
      <Suspense fallback={<div>Loading screen</div>}>
        {registry[view].render(false)}
      </Suspense>
    </Boundary>
  );
}

beforeEach(() => {
  routeModules.capture.mockReset().mockResolvedValue({ CaptureScreen: component("Capture") });
  routeModules.devices.mockReset().mockResolvedValue({ DevicesScreen: component("Devices") });
  routeModules.history.mockReset().mockResolvedValue({ LibraryScreen: component("Library") });
  routeModules.settings.mockReset().mockResolvedValue({ SettingsScreen: component("Settings") });
});

describe("lazy screen registry", () => {
  test("keeps the loading state until the selected route module resolves", async () => {
    let resolve!: (module: { DevicesScreen: ComponentType }) => void;
    routeModules.devices.mockReturnValueOnce(new Promise((done) => {
      resolve = done;
    }));
    const registry = await loadRegistry();
    render(<Route registry={registry} view="devices" />);

    expect(screen.getByText("Loading screen")).toBeTruthy();
    resolve({ DevicesScreen: component("Devices") });
    expect(await screen.findByText("Devices")).toBeTruthy();
  });

  test("switches between independently lazy Devices and Library screens", async () => {
    const registry = await loadRegistry();
    const view = render(<Route registry={registry} view="devices" />);
    expect(await screen.findByText("Devices")).toBeTruthy();

    view.rerender(<Route registry={registry} view="history" />);
    expect(await screen.findByText("Library")).toBeTruthy();
    expect(screen.queryByText("Devices")).toBeNull();
  });
});
