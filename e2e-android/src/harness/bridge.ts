/**
 * The seam the browser layer reaches through `app.daemon`.
 *
 * Android runs no daemon and ships no CLI: the core is linked in-process and
 * the Tauri bridge is the only route to the store (ADR-0003). That is also why
 * seeding through it is a fixture rather than a back door — it is the same
 * command the screen calls, so an item seeded here arrives exactly as one the
 * user copied would.
 *
 * Every helper loops *inside* one `evaluate`. A round trip per item runs over
 * an adb-forwarded CDP socket, and 150 of them is the difference between a
 * second and a minute.
 */
import type { AndroidApp } from "./app.js";

export interface StoredItem {
  id: string;
  content: string;
  pinned: boolean;
  is_sensitive: boolean;
}

interface Internals {
  __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> };
}

export async function invoke<T>(
  app: AndroidApp,
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  return app.withPage((page) =>
    page.evaluate(
      (name, payload) =>
        (window as unknown as Internals).__TAURI_INTERNALS__.invoke(name, payload) as Promise<T>,
      command,
      args,
    ),
  ) as Promise<T>;
}

export async function addItems(app: AndroidApp, contents: string[]): Promise<string[]> {
  return app.withPage((page) =>
    page.evaluate(async (items: string[]) => {
      const { invoke } = (window as unknown as Internals).__TAURI_INTERNALS__;
      const ids: string[] = [];
      for (const content of items) {
        ids.push(((await invoke("add_item", { content })) as { id: string }).id);
      }
      return ids;
    }, contents),
  );
}

/** Ignores an id the store no longer holds: the screen may have deleted it
 *  already, and a cleanup that throws for that hides the real failure. */
export async function deleteItems(app: AndroidApp, ids: string[]): Promise<void> {
  await app.withPage((page) =>
    page.evaluate(async (doomed: string[]) => {
      const { invoke } = (window as unknown as Internals).__TAURI_INTERNALS__;
      for (const id of doomed) {
        try {
          await invoke("delete_item", { id });
        } catch {
          /* already gone */
        }
      }
    }, ids),
  );
}

/** What the store holds, read past the screen. `limit` is above `PAGE_SIZE`,
 *  so a caller counting rows is not counting a page boundary. */
export async function storedItems(app: AndroidApp, limit = 500): Promise<StoredItem[]> {
  const page = await invoke<{ items: StoredItem[] }>(app, "list", { limit, cursor: null });
  return page.items;
}
