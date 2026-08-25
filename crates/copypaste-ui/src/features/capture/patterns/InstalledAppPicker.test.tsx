import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { InstalledAppPicker } from "./InstalledAppPicker";

describe("InstalledAppPicker", () => {
  it("uses the shared failure notice and keeps retry actionable", async () => {
    const user = userEvent.setup();
    const retry = vi.fn();

    render(
      <TooltipProvider>
        <InstalledAppPicker
          apps={[]}
          selectedIds={new Set()}
          query=""
          disabled={false}
          loading={false}
          refreshing={false}
          failed
          onQueryChange={vi.fn()}
          onRetry={retry}
          onAdd={vi.fn()}
        />
      </TooltipProvider>,
    );

    expect(screen.getByRole("alert").textContent).toContain(
      "Installed applications couldn't be read.",
    );
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(retry).toHaveBeenCalledOnce();
  });
});
