/**
 * Android runs no daemon or CLI: the linked core's Tauri bridge is its store
 * seam (ADR-0003). Seeding therefore uses the same command as the product.
 *
 * Batch helpers loop inside one evaluation because each CDP round trip crosses
 * an adb-forwarded socket; 150 separate evaluations take about a minute.
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
