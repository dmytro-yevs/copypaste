import { beforeEach, describe, expect, it, vi } from "vitest";

const call = vi.hoisted(() => vi.fn());
const hasNativeBridge = vi.hoisted(() => vi.fn());

vi.mock("@/lib/ipcCall", () => ({ call, hasNativeBridge }));

import { UI_COMMANDS } from "@/generated/ipc";
import { checkForUpdate, UPDATE_CHECK_TIMEOUT_MS } from "./updater";

beforeEach(() => {
  call.mockReset().mockResolvedValue({ state: "up_to_date" });
  hasNativeBridge.mockReset().mockReturnValue(true);
});

describe("checkForUpdate", () => {
  it("uses its bounded IPC deadline instead of the generic five-minute timeout", async () => {
    await expect(checkForUpdate()).resolves.toEqual({ state: "up_to_date" });

    expect(call).toHaveBeenCalledWith(UI_COMMANDS.check_for_update, undefined, {
      timeoutMs: UPDATE_CHECK_TIMEOUT_MS,
    });
  });
});
