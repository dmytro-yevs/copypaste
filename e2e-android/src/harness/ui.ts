import { sleep } from "./adb.js";
import type { AndroidApp } from "./app.js";

export const NAV = 'nav[aria-label="Primary"]';
export const HISTORY_LIST = '[role="list"][aria-label="Clipboard history"]';
export const ROW = `${HISTORY_LIST} [role="listitem"]`;
export const SEARCH = '[aria-label="Search clipboard history"]';
export const MASKED_ROW = '[aria-label="Sensitive item, hidden — activate to reveal"]';

export async function visibleText(app: AndroidApp): Promise<string> {
  return app.withPage((page) => page.evaluate(() => document.body.innerText));
}

export async function count(app: AndroidApp, selector: string): Promise<number> {
  return app.withPage((page) =>
    page.evaluate((query) => document.querySelectorAll(query).length, selector),
  );
}

export async function rowCount(app: AndroidApp): Promise<number> {
  return count(app, ROW);
}

/** `message` may be a function so a caller can report the last thing it saw
 *  rather than only what it wanted. */
export async function waitFor(
  predicate: () => Promise<boolean>,
  message: string | (() => string),
  timeout = 30_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await sleep(250);
  }
  throw new Error(typeof message === "function" ? message() : message);
}

/** Against rendered text, never a catalogue key — a test that matched a key
 *  would keep passing while the screen showed one. */
export async function waitForText(
  app: AndroidApp,
  needle: string,
  timeout = 30_000,
): Promise<void> {
  await waitFor(
    async () => (await visibleText(app)).includes(needle),
    `never rendered ${JSON.stringify(needle)}`,
    timeout,
  );
}

/**
 * Wait for a seeded row's text with the viewport put back to the top on every
 * attempt.
 *
 * The list is virtualised, so a row that exists is in the document only while
 * the window is over it — and the window moves on its own: a previous file's
 * cleanup deleting a hundred rows re-lays the list out underneath. Scrolling
 * once before waiting is not enough, because the move can come afterwards.
 */
export async function waitForTopRowText(
  app: AndroidApp,
  needle: string,
  timeout = 60_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  for (let attempt = 0; Date.now() < deadline; attempt += 1) {
    // Remount before trying again. Scrolling is not always enough: the list is
    // an infinite query holding every page it has fetched, and after a large
    // delete those pages can go on describing the list as it was until
    // something refetches the first one from scratch.
    if (attempt > 0) {
      await gotoView(app, "Devices");
      await gotoView(app, "History");
    }
    try {
      await waitFor(
        async () => {
          await scrollListToTop(app);
          return (await visibleText(app)).includes(needle);
        },
        "not on screen yet",
        Math.max(2_000, Math.min(15_000, deadline - Date.now())),
      );
      return;
    } catch {
      /* remount and look again */
    }
  }
  throw new Error(`never rendered ${JSON.stringify(needle)} at the top of the list`);
}

export async function waitForRows(
  app: AndroidApp,
  atLeast = 1,
  timeout = 60_000,
): Promise<void> {
  await waitFor(
    async () => (await rowCount(app)) >= atLeast,
    `fewer than ${atLeast} rows ever rendered`,
    timeout,
  );
}

/** Switch screens the way a user does — see `tapWhere` for why the tap is
 *  dispatched at a point this harness computes rather than by `click()`. */
export async function gotoView(app: AndroidApp, label: string): Promise<void> {
  await tapButton(app, label, { within: NAV });
  await waitFor(
    async () =>
      app.withPage((page) =>
        page.evaluate(
          (nav: string, name: string) =>
            Array.from(document.querySelectorAll(`${nav} button`)).some(
              (node) =>
                node.textContent?.trim() === name &&
                node.getAttribute("aria-current") === "page",
            ),
          NAV,
          label,
        ),
      ),
    `the ${label} screen never became current`,
  );
}

/**
 * The list is virtualised, so a row that exists is not a row that is in the
 * document. Newest-first puts a just-captured item at the top, and only there
 * is "did it arrive" a question the DOM can answer.
 */
export async function scrollListToTop(app: AndroidApp): Promise<void> {
  await app.withPage((page) =>
    page.evaluate((selector) => {
      const list = document.querySelector(selector) as HTMLElement | null;
      if (!list) return;
      list.scrollTop = 0;
      list.dispatchEvent(new Event("scroll", { bubbles: true }));
    }, HISTORY_LIST),
  );
}

/**
 * Put the toolbar back to showing everything, newest first.
 *
 * Nothing restarts the app between test files or between runs, so the toolbar
 * keeps whatever the last one left in it. A kind filter still set to Links
 * hides every plain clipping the next file seeds, and the failure it produces
 * says the item was never ingested.
 */
export async function resetHistoryFilters(app: AndroidApp): Promise<void> {
  await clearField(app, SEARCH);
  await app.withPage((page) =>
    page.evaluate(() => {
      for (const label of ["Filter by kind", "Sort order"]) {
        const select = document.querySelector(
          `[aria-label="${label}"]`,
        ) as HTMLSelectElement | null;
        const first = select?.options[0]?.value;
        if (!select || first === undefined || select.value === first) continue;
        select.value = first;
        select.dispatchEvent(new Event("change", { bubbles: true }));
      }
    }),
  );
}

/**
 * Whether the newest row is a masked sensitive one.
 *
 * Asked of the top row rather than by counting masked rows, because the
 * virtualiser renders a fixed window: an item arriving at the top evicts one at
 * the bottom, so a count is unchanged whenever the evicted row was masked too.
 */
export async function topRowIsMasked(app: AndroidApp): Promise<boolean> {
  return app.withPage((page) =>
    page.evaluate(
      (row, masked) => {
        const first = document.querySelector(row);
        return !!first && (first.matches(masked) || first.querySelector(masked) !== null);
      },
      ROW,
      MASKED_ROW,
    ),
  );
}

export interface LabelledBox {
  tag: string;
  width: number;
  height: number;
  text: string;
}

/** Every element carrying this accessible name, with the box it was laid out
 *  at — so "present" and "rendered" are told apart. The query ignores CSS, so a
 *  control hidden with `display: none` is still counted. */
export async function byLabel(app: AndroidApp, label: string): Promise<LabelledBox[]> {
  return app.withPage((page) =>
    page.evaluate(
      (name: string) =>
        Array.from(document.querySelectorAll(`[aria-label="${name}"]`), (node) => {
          const el = node as HTMLElement;
          const rect = el.getBoundingClientRect();
          return { tag: el.tagName, width: rect.width, height: rect.height, text: el.innerText };
        }),
      label,
    ),
  );
}

/**
 * A real tap at a point taken from the live page and checked against
 * `document.elementFromPoint` first, so a control that something else covers
 * fails here rather than being tapped through its cover. That check has
 * already earned its place: seven settings tabs whose boxes were all correct
 * and non-overlapping still had one whose centre belonged to its neighbour's
 * overflowing label.
 *
 * `ElementHandle.click` would be the obvious way to do this and is not usable
 * here. It intersects the element's quads with `Page.getLayoutMetrics`, and
 * this WebView resizes under the insets — a dialog button 60px above the
 * bottom of a 1111px viewport was rejected as "not clickable" against metrics
 * still describing 915.
 */
async function tapWhere(
  app: AndroidApp,
  scope: string | null,
  selector: string,
  label: string | null,
  index: number,
): Promise<boolean> {
  return app.withPage(async (page) => {
    const point = await page.evaluate(
      (root: string | null, query: string, name: string | null, nth: number) => {
        const within = root ? document.querySelector(root) : document;
        if (!within) return null;
        const matches = Array.from(within.querySelectorAll(query)).filter((node) => {
          if (name === null) return true;
          const el = node as HTMLElement;
          return el.textContent?.trim() === name || el.getAttribute("aria-label") === name;
        });
        // A negative index means "the first one a tap can actually reach":
        // the list is virtualised, so its first row in document order may be
        // scrolled under the toolbar while four identical controls below it
        // are on screen.
        const candidates = nth < 0 ? matches : matches.slice(nth, nth + 1);
        for (const node of candidates) {
          const target = node as HTMLElement;
          const rect = target.getBoundingClientRect();
          if (rect.width === 0 || rect.height === 0) continue;
          const x = rect.x + rect.width / 2;
          const y = rect.y + rect.height / 2;
          if (target.contains(document.elementFromPoint(x, y))) return { x, y };
        }
        return null;
      },
      scope,
      selector,
      label,
      index,
    );
    if (!point) return false;
    await page.mouse.click(point.x, point.y);
    return true;
  });
}

export async function tapButton(
  app: AndroidApp,
  label: string,
  options: { within?: string; timeout?: number } = {},
): Promise<void> {
  const { within, timeout = 15_000 } = options;
  await waitFor(
    () => tapWhere(app, within ?? null, "button", label, -1),
    `no tappable button labelled ${JSON.stringify(label)}${within ? ` inside ${within}` : ""}`,
    timeout,
  );
}

/** The nth match of a selector, for controls a label cannot tell apart — the
 *  per-row selection checkboxes are all named the same thing. */
export async function tapNth(
  app: AndroidApp,
  selector: string,
  index: number,
  timeout = 15_000,
): Promise<void> {
  await waitFor(
    () => tapWhere(app, null, selector, null, index),
    `no tappable ${selector} at index ${index}`,
    timeout,
  );
}

export async function fieldValue(app: AndroidApp, selector: string): Promise<string> {
  return app.withPage((page) =>
    page.evaluate((query) => {
      const node = document.querySelector(query) as HTMLInputElement | null;
      return node?.value ?? "";
    }, selector),
  );
}

/** Tap, then type on the keyboard — the mobile path to a text field, and the
 *  one that proves the WebView takes key input at all. */
export async function typeInto(
  app: AndroidApp,
  selector: string,
  text: string,
): Promise<void> {
  await app.withPage(async (page) => {
    await page.click(selector);
    await page.keyboard.type(text, { delay: 20 });
  });
}

/**
 * Backspace from the end rather than select-all: a triple click selects a word
 * on some engines and the whole value on others, and the difference is a
 * half-cleared filter that the next assertion reads as a missing item.
 */
export async function clearField(app: AndroidApp, selector: string): Promise<void> {
  const current = await fieldValue(app, selector);
  if (!current) return;
  await app.withPage(async (page) => {
    await page.click(selector);
    await page.keyboard.press("End");
    for (let i = 0; i < current.length; i++) await page.keyboard.press("Backspace");
  });
  await waitFor(
    async () => (await fieldValue(app, selector)) === "",
    `${selector} still holds text after clearing it`,
  );
}
