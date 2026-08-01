/** jsdom has no layout: where a test needs a box to have a size it says so
 *  explicitly rather than pretending jsdom measured one. */
import type { ReactElement, ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type {
  CaptureSnapshot,
  Item,
  ItemPage,
  PeerInfo,
  ShizukuProbe,
  StatusData,
} from "@/lib/ipc";

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
    origin_device_id: "device-1",
    origin_device_name: "This Mac",
    source_app_bundle_id: null,
    source_app_name: null,
    too_large_to_sync: false,
    ...over,
  };
}

export function items(count: number, over: Partial<Item> = {}): Item[] {
  return Array.from({ length: count }, (_, index) =>
    item({ id: `row-${index}`, content: `entry ${index}`, ...over }),
  );
}

/** `skipped_undecryptable` is always present, including when zero. `nextCursor`
 *  defaults to `null`, the end of the list: a test that wants a second page has
 *  to say so, exactly as the service does — page length does not imply one. */
export function page(
  items: readonly Item[],
  skipped = 0,
  nextCursor: string | null = null,
  total = items.length,
): ItemPage {
  return {
    items: [...items],
    total,
    skipped_undecryptable: skipped,
    next_cursor: nextCursor,
  };
}

export function status(over: Partial<StatusData> = {}): StatusData {
  return {
    version: "2.0.0-alpha.1",
    protocol_version: 1,
    item_count: 3,
    capture_running: true,
    clipboard_backend: "nspasteboard",
    counters: {
      rejected_too_large: 0,
      lost_intermediates: 0,
      sensitive_swept: 0,
      index_purged: 0,
      uptime_secs: 60,
    },
    ...over,
    private_mode: over.private_mode ?? false,
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

export function probe(over: Partial<ShizukuProbe> = {}): ShizukuProbe {
  return {
    supported: true,
    installed: true,
    running: true,
    permission: true,
    toastSuppressed: false,
    rearmRequested: false,
    ...over,
  };
}

/** `headline` and `detail` are quoted from what `capture::messages` produces,
 *  never invented, so a test asserts on what a phone would show. A test needing
 *  a different state supplies both: the view may not derive them, so neither
 *  may the fixture. */
export function captureSnapshot(
  over: Partial<CaptureSnapshot> = {},
): CaptureSnapshot {
  return {
    rung: "shizuku",
    health: { state: "working" },
    shizuku: probe(),
    nextStep: "none",
    headline: "Capturing from every app.",
    detail: null,
    lastReadOkAt: 1_700_000_000_000,
    lastCaptureAt: 1_700_000_000_000,
    droppedClips: 0,
    toastSuppressed: false,
    toastAcknowledged: false,
    rearmRequested: false,
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
