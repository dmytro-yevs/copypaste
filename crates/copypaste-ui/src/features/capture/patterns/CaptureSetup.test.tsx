import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { captureSnapshot, withClient } from "@/test/harness";
import { CaptureSetup } from "./CaptureSetup";

describe("CaptureSetup", () => {
  it("uses the shared warning notice for dropped captures", () => {
    withClient(
      <CaptureSetup
        snapshot={captureSnapshot({ rung: "desktop", droppedClips: 2 })}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain(
      "2 copies were captured but couldn't be saved.",
    );
  });
});
