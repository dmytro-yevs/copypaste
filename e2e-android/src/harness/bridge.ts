/**
 * Android runs no daemon or CLI: the linked core's Tauri bridge is its store
 * seam (ADR-0003). Seeding therefore uses the same command as the product.
 *
 * Batch helpers loop inside one evaluation because each CDP round trip crosses
 * an adb-forwarded socket; 150 separate evaluations take about a minute.
 */
import type { AndroidApp } from "./app.js";
import type {
  Item,
  ItemPage,
} from "../../../crates/copypaste-ui/src/generated/ipc.js";

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

export interface DeleteReport {
  deleted: string[];
  /** The screen deleted it first, which is an outcome and not a failure. */
  alreadyGone: string[];
  failed: { id: string; code: string }[];
}

/**
 * `delete_item` rejects with a serialised `UiError` — `{ code, retryable }`,
 * never prose (`backend/error.rs`) — and an id the store does not hold is
 * exactly `not_found` (`commands/history.rs`: "An unknown id is a not-found
 * failure, not a quiet `false`"). Every other code is the store refusing a
 * delete it was able to attempt.
 */
const NOT_FOUND = "not_found";

/**
 * One evaluation for the whole set, and every id attempted even after one
 * refuses: a cleanup that stopped at the first failure would leave the rest of
 * the fixtures behind, and the seeded credential is one of them.
 *
 * The `catch {}` this replaces could not tell `not_found` from a store that
 * refused the delete, so a cleanup that removed nothing at all was
 * indistinguishable from one that had nothing left to remove — and the fixture
 * `secretFor()` mints stayed in the history of every later run.
 */
export async function deleteItems(app: AndroidApp, ids: string[]): Promise<DeleteReport> {
  const report = await app.withPage((page) =>
    page.evaluate(async (doomed: string[]) => {
      const { invoke } = (window as unknown as Internals).__TAURI_INTERNALS__;
      const outcomes: { id: string; code: string | null }[] = [];
      for (const id of doomed) {
        try {
          await invoke("delete_item", { id });
          outcomes.push({ id, code: null });
        } catch (error) {
          const code = (error as { code?: unknown })?.code;
          outcomes.push({ id, code: typeof code === "string" && code ? code : String(error) });
        }
      }
      return outcomes;
    }, ids),
  );

  const summary: DeleteReport = { deleted: [], alreadyGone: [], failed: [] };
  for (const { id, code } of report) {
    if (code === null) summary.deleted.push(id);
    else if (code === NOT_FOUND) summary.alreadyGone.push(id);
    else summary.failed.push({ id, code });
  }
  if (summary.failed.length) throw new Error(describeDeleteFailures(summary));
  return summary;
}

export function describeDeleteFailures(report: DeleteReport): string {
  const refused = report.failed.map(({ id, code }) => `${id} (${code})`).join(", ");
  return (
    `the store refused ${report.failed.length} of ${
      report.deleted.length + report.alreadyGone.length + report.failed.length
    } fixture delete(s): ${refused}. ` +
    `${report.deleted.length} deleted, ${report.alreadyGone.length} already gone.`
  );
}

/**
 * Teardown's call. An app that is no longer reachable is already the failing
 * test's reason and this must not replace it — but a store that refused a delete
 * it could attempt is this harness's own failure and is raised.
 */
export async function cleanUpItems(app: AndroidApp, ids: string[]): Promise<void> {
  try {
    await deleteItems(app, ids);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("the store refused")) throw error;
    console.warn(`fixture cleanup could not reach the app: ${String(error)}`);
  }
}

export function missingFixtureIds(
  items: readonly Item[],
  fixtureIds: readonly string[],
): string[] {
  const stored = new Set(items.map(({ id }) => id));
  return fixtureIds.filter((id) => !stored.has(id));
}

/** What the store holds, read past the screen. `limit` is above `PAGE_SIZE`,
 *  so a caller counting rows is not counting a page boundary. */
export async function storedItems(app: AndroidApp, limit = 500): Promise<Item[]> {
  const page = await invoke<ItemPage>(app, "list", { limit, cursor: null });
  return page.items;
}
