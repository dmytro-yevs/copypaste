import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
const currentPlatform = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@/lib/platform", () => ({ currentPlatform }));

import { UI_COMMANDS } from "@/generated/ipc";
import { checkForUpdate, UPDATE_CHECK_TIMEOUT_MS } from "./updater";

beforeEach(() => {
  invoke.mockReset();
  currentPlatform.mockReset().mockReturnValue("android");
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
});

afterEach(() => {
  vi.useRealTimers();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("checkForUpdate", () => {
  it("stops waiting at its own deadline when the native command hangs", async () => {
    vi.useFakeTimers();
    invoke.mockReturnValue(new Promise(() => {}));

    const outcome = checkForUpdate();
    const rejection = expect(outcome).rejects.toMatchObject({
      code: "timeout",
      retryable: true,
    });
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_TIMEOUT_MS);

    await rejection;
    expect(invoke).toHaveBeenCalledWith(UI_COMMANDS.check_for_update, undefined);
  });
});
