import { afterEach, beforeEach, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { getClipboardWriteAvailability } from "./ipc";

beforeEach(() => {
  invoke.mockReset();
  invoke
    .mockResolvedValueOnce("available")
    .mockResolvedValueOnce("unsupported_content_type");
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

it("passes the default and explicit clipboard write modes to native", async () => {
  await expect(getClipboardWriteAvailability("text")).resolves.toBe("available");
  await expect(getClipboardWriteAvailability("image/png", "plain_text")).resolves.toBe(
    "unsupported_content_type",
  );

  expect(invoke).toHaveBeenNthCalledWith(1, "clipboard_write_availability", {
    contentType: "text",
    mode: "original",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "clipboard_write_availability", {
    contentType: "image/png",
    mode: "plain_text",
  });
});
