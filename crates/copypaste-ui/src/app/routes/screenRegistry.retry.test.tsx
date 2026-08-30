import { Suspense, type ComponentType } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

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

function component(label: string): ComponentType {
  return () => <div>{label}</div>;
}

test("retries a rejected route with a fresh lazy import", async () => {
  vi.spyOn(console, "error").mockImplementation(() => undefined);
  routeModules.capture.mockResolvedValue({ CaptureScreen: component("Capture") });
  routeModules.history.mockResolvedValue({ LibraryScreen: component("Library") });
  routeModules.settings.mockResolvedValue({ SettingsScreen: component("Settings") });
  let reject = true;
  routeModules.devices.mockImplementation(async () => {
    if (reject) throw new Error("/private/path/devices.js failed");
    return { DevicesScreen: component("Devices") };
  });
  const registry = (await import("./screenRegistry")).screenRegistry;
  render(
    <Boundary label="Devices" onReset={registry.devices.reset}>
      <Suspense fallback={<div>Loading screen</div>}>
        {registry.devices.render(false)}
      </Suspense>
    </Boundary>,
  );

  expect(await screen.findByText("Devices didn’t open")).toBeTruthy();
  expect(document.body.textContent).not.toContain("/private/path/devices.js");
  const rejectedAttempts = routeModules.devices.mock.calls.length;
  reject = false;
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  expect(await screen.findByText("Devices")).toBeTruthy();
  expect(routeModules.devices.mock.calls.length).toBeGreaterThan(rejectedAttempts);
  expect(routeModules.history).not.toHaveBeenCalled();
});
