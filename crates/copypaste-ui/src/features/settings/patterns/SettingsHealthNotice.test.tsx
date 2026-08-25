import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SettingsHealth } from "@/generated/ipc";
import { SettingsHealthNotice } from "./SettingsHealthNotice";

const status = vi.hoisted(() => ({
  health: null as SettingsHealth | null,
}));

vi.mock("@/hooks/useStatus", () => ({
  useStatus: () => ({ data: status.health }),
}));

describe("SettingsHealthNotice", () => {
  beforeEach(() => {
    status.health = null;
  });

  it("keeps degraded settings feedback nonblocking", () => {
    status.health = {
      record_unreadable: false,
      unreadable_fields: ["private_mode"],
    };

    render(<SettingsHealthNotice />);

    const title = screen.getByText("Some saved settings couldn't be read.");
    const notice = title.closest("[aria-live=polite]");
    expect(notice).not.toBeNull();
    expect(notice?.getAttribute("role")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(/CopyPaste chose the safer setting/)).toBeTruthy();
  });
});
