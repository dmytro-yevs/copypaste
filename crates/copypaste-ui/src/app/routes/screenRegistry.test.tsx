import { Suspense, type ComponentType } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { Boundary } from "@/app/shell/Boundary";
import { createScreenRegistry } from "./screenRegistry";

function moduleWith(label: string): Promise<{ default: ComponentType }> {
  return Promise.resolve({ default: () => <div>{label}</div> });
}

function loaders(history = moduleWith("Library"), devices = moduleWith("Devices")) {
  return {
    history: async () => history,
    devices: async () => devices,
    settings: async () => moduleWith("Settings"),
    capture: async () => moduleWith("Capture"),
  };
}

function Route({ registry, view }: { registry: ReturnType<typeof createScreenRegistry>; view: "history" | "devices" }) {
  return (
    <Boundary label={view === "devices" ? "Devices" : "Library"} onReset={registry[view].reset}>
      <Suspense fallback={<div>Loading screen</div>}>
        {registry[view].render(false)}
      </Suspense>
    </Boundary>
  );
}

afterEach(() => vi.restoreAllMocks());

describe("lazy screen registry", () => {
  test("keeps the loading state until the selected route module resolves", async () => {
    let resolve!: (module: { default: ComponentType }) => void;
    const pending = new Promise<{ default: ComponentType }>((done) => {
      resolve = done;
    });
    const registry = createScreenRegistry(loaders(undefined, pending));
    render(<Route registry={registry} view="devices" />);

    expect(screen.getByText("Loading screen")).toBeTruthy();
    resolve({ default: () => <div>Devices</div> });
    expect(await screen.findByText("Devices")).toBeTruthy();
  });

  test("retries a rejected route with a fresh lazy import", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const devices = vi.fn(async () => {
      if (devices.mock.calls.length === 1) throw new Error("/private/path/devices.js failed");
      return { default: () => <div>Devices</div> };
    });
    const history = vi.fn(async () => ({ default: () => <div>Library</div> }));
    const registry = createScreenRegistry({ ...loaders(), history, devices });
    render(<Route registry={registry} view="devices" />);

    expect(await screen.findByText("Devices didn’t open")).toBeTruthy();
    expect(document.body.textContent).not.toContain("/private/path/devices.js");
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Devices")).toBeTruthy();
    expect(devices).toHaveBeenCalledTimes(2);
    expect(history).not.toHaveBeenCalled();
  });

  test("switches between independently lazy Devices and Library screens", async () => {
    const registry = createScreenRegistry(loaders());
    const view = render(<Route registry={registry} view="devices" />);
    expect(await screen.findByText("Devices")).toBeTruthy();

    view.rerender(<Route registry={registry} view="history" />);
    expect(await screen.findByText("Library")).toBeTruthy();
    expect(screen.queryByText("Devices")).toBeNull();
  });
});
