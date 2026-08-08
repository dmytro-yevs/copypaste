import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import { useHistory } from "@/hooks/useHistory";
import { useOpenAtLogin } from "@/hooks/useOpenAtLogin";
import {
  EVENT_AUTOSTART_CHANGED,
  EVENT_CHANGED,
  usePush,
  type ChangePayload,
} from "@/hooks/usePush";
import { PAGE_SIZE } from "@/lib/layout";
import { items, page, testClient } from "@/test/harness";

const listItems = vi.fn();
const searchItems = vi.fn();
const getOpenAtLogin = vi.fn();
const hasBridge = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...args: unknown[]) => listItems(...args),
    searchItems: (...args: unknown[]) => searchItems(...args),
    getOpenAtLogin: () => getOpenAtLogin(),
    hasBridge: () => hasBridge(),
  };
});

type Handler = (event: { payload: never }) => void;
const handlers = new Map<string, Handler[]>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, handler: Handler) => {
    const held = handlers.get(name) ?? [];
    held.push(handler);
    handlers.set(name, held);
    return Promise.resolve(() => {
      handlers.set(
        name,
        (handlers.get(name) ?? []).filter((entry) => entry !== handler),
      );
    });
  },
}));

function emitChanged(payload: ChangePayload) {
  for (const handler of handlers.get(EVENT_CHANGED) ?? []) {
    handler({ payload: payload as never });
  }
}

function emitAutostartChanged(payload: boolean) {
  for (const handler of handlers.get(EVENT_AUTOSTART_CHANGED) ?? []) {
    handler({ payload: payload as never });
  }
}

function threePages() {
  const first = items(PAGE_SIZE);
  const second = items(PAGE_SIZE).map((entry) => ({ ...entry, id: `p2-${entry.id}` }));
  const third = items(PAGE_SIZE).map((entry) => ({ ...entry, id: `p3-${entry.id}` }));
  return { first, second, third };
}

function mounted(client = testClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(
    () => {
      usePush();
      return useHistory("");
    },
    { wrapper: Wrapper },
  );
}

function mountedOpenAtLogin(client = testClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(
    () => {
      usePush();
      return useOpenAtLogin();
    },
    { wrapper: Wrapper },
  );
}

beforeEach(() => {
  handlers.clear();
  listItems.mockReset();
  searchItems.mockReset();
  getOpenAtLogin.mockReset();
  hasBridge.mockReset().mockReturnValue(true);
});

describe("native launch-at-login changes", () => {
  it("re-reads the system state instead of trusting the event payload", async () => {
    let systemState = false;
    getOpenAtLogin.mockImplementation(async () => systemState);

    const { result } = mountedOpenAtLogin();
    await waitFor(() => expect(result.current.data).toBe(false));

    systemState = true;
    emitAutostartChanged(false);

    await waitFor(() => expect(result.current.data).toBe(true));
    expect(getOpenAtLogin).toHaveBeenCalledTimes(2);
  });
});

afterEach(() => vi.restoreAllMocks());

describe("manifest 05 §5.4: a remote change three pages down still reaches the window", () => {
  it("shows a remote delete of a page-3 item with no scroll and no refocus", async () => {
    const { first, second } = threePages();
    let third = threePages().third;
    const doomed = third[0].id;

    listItems.mockImplementation(async (_limit: number, cursor: string | null) => {
      if (cursor === null) return page(first, 0, "cursor-1");
      if (cursor === "cursor-1") return page(second, 0, "cursor-2");
      return page(third);
    });

    const { result } = mounted();
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE));
    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 2));
    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 3));
    expect(result.current.data?.items.some((entry) => entry.id === doomed)).toBe(true);

    third = third.slice(1);
    emitChanged({ topic: "items", item_count: PAGE_SIZE * 3 - 1 });

    await waitFor(() =>
      expect(result.current.data?.items.some((entry) => entry.id === doomed)).toBe(false),
    );
    expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 3 - 1);
  });

  it("shows a remote pin of a page-3 item with no scroll and no refocus", async () => {
    const { first, second } = threePages();
    let third = threePages().third;
    const target = third[2].id;

    listItems.mockImplementation(async (_limit: number, cursor: string | null) => {
      if (cursor === null) return page(first, 0, "cursor-1");
      if (cursor === "cursor-1") return page(second, 0, "cursor-2");
      return page(third);
    });

    const { result } = mounted();
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE));
    await result.current.fetchNextPage();
    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 3));
    expect(
      result.current.data?.items.find((entry) => entry.id === target)?.pinned,
    ).toBe(false);

    third = third.map((entry) =>
      entry.id === target ? { ...entry, pinned: true, pin_order: 1 } : entry,
    );
    emitChanged({ topic: "items", item_count: PAGE_SIZE * 3 });

    await waitFor(() =>
      expect(
        result.current.data?.items.find((entry) => entry.id === target)?.pinned,
      ).toBe(true),
    );
  });
});
