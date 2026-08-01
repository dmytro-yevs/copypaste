import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import { item, testClient } from "@/test/harness";

const toast = vi.fn();
const deleteItem = vi.fn();

vi.mock("sonner", () => ({ toast: (...args: unknown[]) => toast(...args) }));
vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, deleteItem: (...args: unknown[]) => deleteItem(...args) };
});

function wrapper() {
  const client = testClient();
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

beforeEach(() => {
  toast.mockReset();
  deleteItem.mockReset().mockResolvedValue(undefined);
});

afterEach(() => vi.restoreAllMocks());

describe("single-item delete feedback", () => {
  it("shows only Deleted with an inline Undo action, never the clip or its id", () => {
    const clip = item({ id: "private-item-id", content: "top secret clipboard text" });
    const { result } = renderHook(() => useDeferredDelete(), { wrapper: wrapper() });

    act(() => result.current.remove(clip));

    const [title, options] = toast.mock.calls[0] ?? [];
    expect(title).toBe("Deleted");
    expect(options).toEqual(expect.objectContaining({
      action: expect.objectContaining({ label: "Undo" }),
    }));
    expect(options).not.toHaveProperty("description");
    expect(JSON.stringify([title, options])).not.toContain(clip.content);
    expect(JSON.stringify([title, options])).not.toContain(clip.id);

    act(() => options.action.onClick());
  });
});
