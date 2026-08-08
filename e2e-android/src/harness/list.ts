/**
 * The virtualised list's geometry, read the way `e2e/src/harness/ui.ts` reads
 * it after 6e9d7b7f.
 *
 * The scroll offset and the rendered row window come from ONE evaluation. Read
 * in two round trips they describe states that never coexisted: a batched
 * delete moves the list through a dozen sizes, and the pair that comes back
 * then fails an invariant the app never violated. The frame counter is what
 * tells "unchanged" apart from "not repainted yet".
 */
import { HISTORY_LIST, ROW } from "./ui.js";
import type { AndroidApp } from "./app.js";

export interface RowBox {
  id: string;
  /** `translateY` from the virtualiser, i.e. the row's offset in list space. */
  start: number;
  height: number;
  active: boolean;
  text: string;
}

export interface ListSnapshot {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  /** The virtualiser's spacer, i.e. the height it reserves for the whole list. */
  totalSize: number;
  rows: RowBox[];
  /** Rendering opportunities since the first snapshot of this session. */
  frame: number;
}

export async function listSnapshot(app: AndroidApp): Promise<ListSnapshot> {
  return app.withPage((page) =>
    page.evaluate(
      (listSelector: string, rowSelector: string) => {
        const win = window as unknown as { __cpFrames?: { n: number } };
        if (!win.__cpFrames) {
          const counter = { n: 0 };
          win.__cpFrames = counter;
          const tick = () => {
            counter.n += 1;
            requestAnimationFrame(tick);
          };
          requestAnimationFrame(tick);
        }
        const el = document.querySelector(listSelector) as HTMLElement | null;
        return {
          frame: win.__cpFrames.n,
          scrollTop: el ? el.scrollTop : NaN,
          scrollHeight: el ? el.scrollHeight : NaN,
          clientHeight: el ? el.clientHeight : NaN,
          totalSize:
            el && el.firstElementChild
              ? el.firstElementChild.getBoundingClientRect().height
              : NaN,
          rows: Array.from(document.querySelectorAll(rowSelector), (node) => {
            const row = node as HTMLElement;
            const match = /translateY\(([-0-9.]+)px\)/.exec(row.style.transform);
            return {
              id: row.id.replace(/^history-row-/, ""),
              start: match && match[1] !== undefined ? parseFloat(match[1]) : NaN,
              height: row.getBoundingClientRect().height,
              active: row.getAttribute("aria-current") === "true",
              text: row.innerText,
            };
          }),
        };
      },
      HISTORY_LIST,
      ROW,
    ),
  );
}

export async function rowBoxes(app: AndroidApp): Promise<RowBox[]> {
  return (await listSnapshot(app)).rows;
}

/** Geometry only. Row text carries a relative age that ticks on its own, so a
 *  signature including it would never repeat and nothing would ever settle. */
function signature(snapshot: ListSnapshot): string {
  const rows = snapshot.rows
    .map((row) => `${row.id}:${Math.round(row.start)}+${Math.round(row.height)}`)
    .join(",");
  return (
    `${Math.round(snapshot.totalSize)}@${Math.round(snapshot.scrollTop)}` +
    `/${Math.round(snapshot.scrollHeight)}x${Math.round(snapshot.clientHeight)}[${rows}]`
  );
}

const FRAMES_AT_REST = 2;

/**
 * The first snapshot that satisfies `predicate` and has held its geometry
 * across at least `FRAMES_AT_REST` rendering opportunities.
 *
 * The spacer takes its new height in the render that received the new items;
 * the row window only moves once the engine has dispatched the scroll event the
 * clamp produced and React has rendered again. Two equal samples do not prove
 * that happened, two with frames between them do.
 */
export async function settledList(
  app: AndroidApp,
  predicate: (snapshot: ListSnapshot) => boolean,
  options: { describe: string; timeout?: number; interval?: number },
): Promise<ListSnapshot> {
  const { describe, timeout = 45_000, interval = 150 } = options;
  const deadline = Date.now() + timeout;
  const seen: string[] = [];
  let previous: ListSnapshot | null = null;

  for (;;) {
    const snapshot = await listSnapshot(app);
    const current = signature(snapshot);
    if (!seen[seen.length - 1]?.startsWith(current)) {
      seen.push(`${current}f${snapshot.frame}`);
    }
    if (
      predicate(snapshot) &&
      previous !== null &&
      signature(previous) === current &&
      snapshot.frame - previous.frame >= FRAMES_AT_REST
    ) {
      return snapshot;
    }
    previous = snapshot;
    if (Date.now() > deadline) {
      throw new Error(
        `${describe}. Observed totalSize@scrollTop/scrollHeightxclientHeight` +
          `[rows]frame, in order: ${seen.join(" ")}`,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, interval));
  }
}

export async function scrollTo(app: AndroidApp, top: number): Promise<void> {
  await app.withPage((page) =>
    page.evaluate(
      (selector: string, offset: number) => {
        const el = document.querySelector(selector) as HTMLElement | null;
        if (!el) return;
        el.scrollTop = offset;
        el.dispatchEvent(new Event("scroll", { bubbles: true }));
      },
      HISTORY_LIST,
      top,
    ),
  );
}
