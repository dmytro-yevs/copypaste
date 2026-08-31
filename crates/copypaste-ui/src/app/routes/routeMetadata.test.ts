import { describe, expect, it } from "vitest";

import { navigationRoutes, routeMetadata } from "./routeMetadata";

describe("route metadata", () => {
  it("owns canonical labels and icons while navigation surfaces choose only order", () => {
    expect(navigationRoutes("sidebar").map((route) => route.view)).toEqual([
      "history",
      "devices",
      "settings",
    ]);
    expect(navigationRoutes("dock").map((route) => route.view)).toEqual([
      "devices",
      "history",
      "settings",
    ]);
    expect(routeMetadata.devices).toMatchObject({
      label: "nav.devices",
      icon: "devices",
    });
    expect(routeMetadata.settings).toMatchObject({
      label: "nav.settings",
      icon: "settings",
    });
  });
});
