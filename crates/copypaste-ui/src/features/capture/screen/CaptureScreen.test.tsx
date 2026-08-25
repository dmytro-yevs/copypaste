import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CaptureScreen } from "./CaptureScreen";

vi.mock("@/features/capture/patterns/CaptureSetup", () => ({
  CaptureSetupState: () => <div>Canonical capture resolver</div>,
}));

describe("CaptureScreen", () => {
  it("delegates loading, error, and data resolution to CaptureSetupState", () => {
    render(<CaptureScreen />);

    expect(screen.getByText("Canonical capture resolver")).toBeTruthy();
  });
});
