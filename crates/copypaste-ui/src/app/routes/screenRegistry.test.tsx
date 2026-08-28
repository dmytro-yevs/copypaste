import { Suspense, type ComponentType } from "react";
import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "react-error-boundary";
import { afterEach, describe, expect, test, vi } from "vitest";

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
    <ErrorBoundary fallback={<div>Screen unavailable</div>}>
      <Suspense fallback={<div>Loading screen</div>}>
        {registry[view].render(false)}
      </Suspense>
    </ErrorBoundary>
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

  test("turns a rejected route import into the safe route boundary", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const rejected = Promise.reject(new Error("/private/path/devices.js failed"));
    const registry = createScreenRegistry(loaders(undefined, rejected));
    render(<Route registry={registry} view="devices" />);

    expect(await screen.findByText("Screen unavailable")).toBeTruthy();
    expect(document.body.textContent).not.toContain("/private/path/devices.js");
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
