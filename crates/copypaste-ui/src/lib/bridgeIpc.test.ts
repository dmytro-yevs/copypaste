import { describe, it, expect, vi, beforeEach } from "vitest";
import { bridgeInvoke } from "./bridgeIpc";

describe("bridgeInvoke", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("routes ipc_call through POST /__ipc and returns the reply", async () => {
    const reply = { ok: true, data: { items: [] } };
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response(JSON.stringify(reply)));
    const out = await bridgeInvoke("ipc_call", {
      method: "history_page",
      params: { limit: 1, offset: 0 },
    });
    expect(out).toEqual(reply);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/__ipc");
    expect(JSON.parse((init as RequestInit).body as string)).toEqual({
      method: "history_page",
      params: { limit: 1, offset: 0 },
    });
  });

  it("stubs OS-side commands without touching the network", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    expect(await bridgeInvoke("app_version")).toBe("dev-bridge");
    expect(await bridgeInvoke("check_accessibility_permission")).toBe(false);
    expect(await bridgeInvoke("play_copy_sound")).toBeUndefined();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
