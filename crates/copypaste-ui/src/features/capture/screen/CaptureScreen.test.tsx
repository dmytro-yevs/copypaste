import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CaptureScreen } from "./CaptureScreen";
import { withClient } from "@/test/harness";
import { useUi } from "@/store/ui";
import { TooltipProvider } from "@/components/ui/tooltip";

vi.mock("@/features/capture/patterns/CaptureSetup", () => ({
  CaptureSetupState: () => <div>Canonical capture resolver</div>,
}));

describe("CaptureScreen", () => {
  it("delegates loading, error, and data resolution to CaptureSetupState", () => {
    withClient(<TooltipProvider><CaptureScreen /></TooltipProvider>);

    expect(screen.getByText("Canonical capture resolver")).toBeTruthy();
  });

  it("returns to Library through the visible back action", async () => {
    useUi.setState({ view: "capture" });
    withClient(<TooltipProvider><CaptureScreen /></TooltipProvider>);
    await userEvent.click(screen.getByRole("button", { name: "Back to Library" }));
    await waitFor(() => expect(useUi.getState().view).toBe("history"));
  });
});
