/**
 * Shared test scaffolding.
 *
 * These tests are the only execution this UI gets: WebKitGTK does not run
 * JavaScript under headless Xvfb without a GPU here, so the app cannot be
 * launched and looked at. Everything below runs in jsdom, which means layout is
 * simulated — where a test needs a box to have a size, it says so explicitly
 * rather than pretending jsdom measured one.
 */
import type { ReactElement, ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { Item, PeerInfo, StatusData } from "@/lib/ipc";

/** A test item. `content: null` is what the bridge sends for a sensitive one —
 *  the plaintext is dropped before it crosses (INV-10). */
export function item(over: Partial<Item> = {}): Item {
  const sensitive = over.is_sensitive ?? false;
  return {
    id: "row-1",
    content: sensitive ? null : "an ordinary clipboard entry",
    content_type: "text/plain",
    created_at: 1_700_000_000_000,
    pinned: false,
    is_sensitive: sensitive,
    ...over,
  };
}

export function items(count: number, over: Partial<Item> = {}): Item[] {
  return Array.from({ length: count }, (_, index) =>
    item({ id: `row-${index}`, content: `entry ${index}`, ...over }),
  );
}

export function status(over: Partial<StatusData> = {}): StatusData {
  return {
    version: "2.0.0-alpha.1",
    protocol_version: 1,
    item_count: 3,
    capture_running: true,
    clipboard_backend: "nspasteboard",
    ...over,
  };
}

export function peer(over: Partial<PeerInfo> = {}): PeerInfo {
  return {
    pairing_id: "pair-1",
    name: "Kitchen Mac",
    last_addr: "192.168.1.24:7420",
    last_seen_ms: 1_700_000_000_000,
    online: true,
    ...over,
  };
}

/** A client with retries off and no background polling, so a test asserts on
 *  what it triggered rather than on a timer. */
export function testClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, refetchInterval: false, gcTime: 0 },
      mutations: { retry: false },
    },
  });
}

/** `withClient` plus a `userEvent` session, for the tests that click. */
export function withUser(ui: ReactElement, client = testClient()) {
  return { user: userEvent.setup(), ...withClient(ui, client) };
}

export function withClient(ui: ReactElement, client = testClient()) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, ...render(ui, { wrapper }) };
}

/** Override the faked scroll extent for one element — used by the tests that
 *  drive load-more, where the distance to the bottom is the assertion. */
export function setScrollHeight(el: HTMLElement, scrollHeight: number) {
  Object.defineProperty(el, "scrollHeight", { configurable: true, value: scrollHeight });
}
