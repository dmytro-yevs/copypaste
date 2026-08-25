import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { withUser } from "@/test/harness";
import { DiagnosticsTab } from "./DiagnosticsTab";

const diagnostics = vi.hoisted(() => ({
  refetch: vi.fn(),
}));

vi.mock("@/hooks/useDiagnostics", () => ({
  useDiagnostics: () => ({
    data: undefined,
    error: { code: "unavailable", retryable: true },
    isFetching: false,
    refetch: diagnostics.refetch,
  }),
  useSweepNotices: vi.fn(),
}));

describe("DiagnosticsTab error state", () => {
  beforeEach(() => {
    diagnostics.refetch.mockReset().mockResolvedValue(undefined);
  });

  it("uses the repair mascot and exposes one retry action", async () => {
    const { user } = withUser(
      <TooltipProvider>
        <DiagnosticsTab />
      </TooltipProvider>,
    );

    const alert = screen.getByRole("alert");
    expect(alert.querySelector("svg")).not.toBeNull();
    expect(screen.getByText("Diagnostics are unavailable.")).toBeTruthy();
    expect(screen.queryByTestId("state-notice")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(diagnostics.refetch).toHaveBeenCalledTimes(1);
  });
});
