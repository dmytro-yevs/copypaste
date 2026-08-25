import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { captureSnapshot } from "@/test/harness";
import { CaptureStatus } from "./CaptureStatus";

const mocks = vi.hoisted(() => ({
  snapshot: undefined as ReturnType<typeof captureSnapshot> | undefined,
  setView: vi.fn(),
}));

vi.mock("@/hooks/useCapture", () => ({
  useCaptureState: () => ({ data: mocks.snapshot }),
}));

vi.mock("@/store/ui", () => ({
  useUi: (select: (state: { setView: typeof mocks.setView }) => unknown) =>
    select({ setView: mocks.setView }),
}));

afterEach(() => {
  mocks.snapshot = undefined;
  mocks.setView.mockReset();
});

describe("CaptureStatus", () => {
  it("uses the same assertive fault semantics as Capture setup", () => {
    mocks.snapshot = captureSnapshot({
      health: { state: "granted_not_working", reason: "read_refused" },
      headline: "Clipboard access was refused.",
      detail: "Copy once, then try again.",
    });
    render(<CaptureStatus />);

    expect(screen.getByRole("alert").getAttribute("aria-live")).toBe(
      "assertive",
    );
  });

  it("keeps setup attention states polite", () => {
    mocks.snapshot = captureSnapshot({
      health: { state: "granted_not_working", reason: "not_armed" },
      headline: "Background capture needs setup.",
    });
    render(<CaptureStatus />);

    expect(screen.getByRole("status").getAttribute("aria-live")).toBe(
      "polite",
    );
  });
});
