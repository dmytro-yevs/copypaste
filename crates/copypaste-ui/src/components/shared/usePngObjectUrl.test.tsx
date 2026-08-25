import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { usePngObjectUrl } from "./usePngObjectUrl";

const createObjectURL = vi.fn<(blob: Blob) => string>();
const revokeObjectURL = vi.fn<(url: string) => void>();

afterEach(() => {
  createObjectURL.mockReset();
  revokeObjectURL.mockReset();
  vi.unstubAllGlobals();
});

function installUrlMocks() {
  let next = 0;
  createObjectURL.mockImplementation(() => `blob:png-${++next}`);
  vi.stubGlobal("URL", {
    ...URL,
    createObjectURL,
    revokeObjectURL,
  });
}

describe("usePngObjectUrl", () => {
  it("distinguishes absent, invalid, and ready input", async () => {
    installUrlMocks();
    const hook = renderHook(
      ({ value }: { value?: string | null }) => usePngObjectUrl(value),
      { initialProps: { value: null as string | null | undefined } },
    );

    expect(hook.result.current.state).toBe("absent");
    hook.rerender({ value: "%%%" });
    await waitFor(() => expect(hook.result.current.state).toBe("invalid"));
    hook.rerender({ value: "iVBORw0KGgo=" });
    await waitFor(() => expect(hook.result.current.state).toBe("ready"));
    expect(createObjectURL).toHaveBeenCalledTimes(1);
  });

  it("revokes each replaced and unmounted URL exactly once", async () => {
    installUrlMocks();
    const hook = renderHook(
      ({ value }: { value: string }) => usePngObjectUrl(value),
      { initialProps: { value: "iVBORw0KGgo=" } },
    );
    await waitFor(() => expect(hook.result.current.state).toBe("ready"));
    const first = hook.result.current.state === "ready"
      ? hook.result.current.url
      : "";

    hook.rerender({ value: "iVBORw0KGgoAAAANSUhEUg==" });
    await waitFor(() => {
      expect(hook.result.current.state).toBe("ready");
      expect(revokeObjectURL).toHaveBeenCalledWith(first);
    });
    const second = hook.result.current.state === "ready"
      ? hook.result.current.url
      : "";

    hook.unmount();
    expect(revokeObjectURL.mock.calls.map(([url]) => url)).toEqual([
      first,
      second,
    ]);
  });

  it("revokes an image that a consumer marks invalid exactly once", async () => {
    installUrlMocks();
    const hook = renderHook(() => usePngObjectUrl("iVBORw0KGgo="));
    await waitFor(() => expect(hook.result.current.state).toBe("ready"));
    const url = hook.result.current.state === "ready"
      ? hook.result.current.url
      : "";

    act(() => hook.result.current.invalidate());
    expect(hook.result.current.state).toBe("invalid");
    expect(revokeObjectURL).toHaveBeenCalledTimes(1);
    expect(revokeObjectURL).toHaveBeenCalledWith(url);
    hook.unmount();
    expect(revokeObjectURL).toHaveBeenCalledTimes(1);
  });
});
