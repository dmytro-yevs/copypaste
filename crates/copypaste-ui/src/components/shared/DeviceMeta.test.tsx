import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/components/ui", () => ({
  Icon: ({ name }: { name: string }) => <span data-icon={name} />,
}));

import { DeviceMeta } from "./DeviceMeta";

describe("DeviceMeta", () => {
  it.each([
    ["desktop", "monitor"],
    ["laptop", "laptop"],
    ["phone", "mobile"],
    ["tablet", "tablet"],
    ["unknown", "devices"],
  ] as const)("renders the generated %s class with the %s glyph", (kind, icon) => {
    const { container } = render(<DeviceMeta label="Paired device" kind={kind} />);
    const meta = container.firstElementChild;

    expect(meta?.getAttribute("data-device-kind")).toBe(kind);
    expect(meta?.querySelector("[data-icon]")?.getAttribute("data-icon")).toBe(icon);
  });

  it("keeps deceptive labels generic without generated device metadata", () => {
    for (const kind of [undefined, "unknown"] as const) {
      const { container, unmount } = render(
        <DeviceMeta label="Windows phone tablet laptop" kind={kind} />,
      );

      expect(container.firstElementChild?.getAttribute("data-device-kind")).toBe("unknown");
      expect(container.querySelector("[data-icon]")?.getAttribute("data-icon")).toBe("devices");
      unmount();
    }
  });
});
