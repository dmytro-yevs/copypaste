import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StateView } from "./StateView";

describe("StateView", () => {
  it("announces errors and preserves caller actions and identifiers", () => {
    render(<StateView mode="error" placement="panel" id="sync-error" title="Sync failed" description="Could not reach the device." actions={<button>Retry</button>} />);
    const alert = screen.getByRole("alert");
    expect(alert.id).toBe("sync-error");
    expect(alert.textContent).toContain("Could not reach the device.");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });

  it("uses the same loading graphic across placements and exposes progress", () => {
    const { container } = render(<><StateView mode="loading" placement="control" title="25%" /><StateView mode="loading" placement="screen" title="25%" /></>);
    const loading = screen.getAllByRole("status");
    expect(loading).toHaveLength(2);
    expect(loading.every((node) => node.getAttribute("aria-busy") === "true")).toBe(true);
    expect(loading.every((node) => node.textContent?.includes("25%"))).toBe(true);
    expect(container.querySelectorAll('[data-mode="loading"] svg')).toHaveLength(2);
  });
});
