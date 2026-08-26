import type { Browser } from "./webview-guard.js";

export const HISTORY_LIST = '[role="list"][aria-label="Clipboard history"]';
export const ROW = '[role="listitem"]';
export const PRIMARY_NAVIGATION =
  'aside[aria-label="Primary"], nav[aria-label="Primary"]';

export interface RowBox {
  id: string;
  /** `translateY` from the virtualiser, i.e. the row's offset in list space. */
  start: number;
  height: number;
  active: boolean;
  text: string;
}

/**
 * Row geometry as the engine actually laid it out — `getBoundingClientRect`,
 * not the inline style — so a row that renders at a different size than it
 * reserved is visible to the assertions.
 */
export async function rowBoxes(browser: Browser): Promise<RowBox[]> {
  return (await listSnapshot(browser)).rows;
}

export interface ScrollBox {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  /** The virtualiser's spacer, i.e. the height it reserves for the whole list. */
  totalSize: number;
}

export interface ListSnapshot extends ScrollBox {
  rows: RowBox[];
  /** Rendering opportunities since the first snapshot of this session. */
  frame: number;
}

export async function scroller(browser: Browser): Promise<ScrollBox> {
  return (await browser.execute(function (selector: string) {
    const el = document.querySelector(selector) as HTMLElement;
    return {
      scrollTop: el.scrollTop,
      scrollHeight: el.scrollHeight,
      clientHeight: el.clientHeight,
      totalSize: el.firstElementChild
        ? el.firstElementChild.getBoundingClientRect().height
        : NaN,
    };
  }, HISTORY_LIST)) as ScrollBox;
}

/**
 * The scroll offset and the rendered row window from one evaluation, plus a
 * count of the rendering opportunities the page has had.
 *
 * `scroller()` then `rowBoxes()` are two round trips and the list can commit a
 * render between them, so the assertions would compare an offset and a row
 * window that never coexisted. The frame counter is what `settledList` needs to
 * tell "unchanged" apart from "not repainted yet".
 */
export async function listSnapshot(browser: Browser): Promise<ListSnapshot> {
  return (await browser.execute(
    function (listSelector: string, rowSelector: string) {
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
        rows: Array.prototype.map.call(
          document.querySelectorAll(rowSelector),
          function (node) {
            const row = node as HTMLElement;
            const rect = row.getBoundingClientRect();
            const match = /translateY\(([-0-9.]+)px\)/.exec(row.style.transform);
            return {
              id: row.id.replace(/^history-row-/, ""),
              start: match && match[1] !== undefined ? parseFloat(match[1]) : NaN,
              height: rect.height,
              active: row.getAttribute("aria-current") === "true",
              text: row.innerText,
            };
          },
        ),
      };
    },
    HISTORY_LIST,
    ROW,
  )) as ListSnapshot;
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
 * the row window only moves once the engine has dispatched the scroll event
 * the clamp produced and React has rendered again. Two equal samples do not
 * prove that happened, two with frames between them do.
 *
 * The settle test is geometry and frames alone, so an assertion that fails
 * after it fails every time.
 */
export async function settledList(
  browser: Browser,
  predicate: (snapshot: ListSnapshot) => boolean,
  options: { describe: string; timeout?: number; interval?: number },
): Promise<ListSnapshot> {
  const { describe, timeout = 45_000, interval = 150 } = options;
  const deadline = Date.now() + timeout;
  const seen: string[] = [];
  let previous: ListSnapshot | null = null;

  for (;;) {
    const snapshot = await listSnapshot(browser);
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

export async function scrollTo(browser: Browser, top: number): Promise<void> {
  await browser.execute(
    function (selector: string, offset: number) {
      const el = document.querySelector(selector) as HTMLElement;
      el.scrollTop = offset;
      el.dispatchEvent(new Event("scroll", { bubbles: true }));
    },
    HISTORY_LIST,
    top,
  );
}

/** How many nodes match, over the whole document and ignoring CSS — so a
 *  control hidden with `display: none` still counts as rendered. */
export async function count(browser: Browser, selector: string): Promise<number> {
  return (await browser.execute(
    (query: string) => document.querySelectorAll(query).length,
    selector,
  )) as number;
}

export async function rowCount(browser: Browser): Promise<number> {
  return count(browser, ROW);
}

export async function waitForRows(
  browser: Browser,
  atLeast = 1,
  timeout = 60_000,
): Promise<void> {
  await browser.waitUntil(
    async () => (await rowCount(browser)) >= atLeast,
    {
      // Generous on purpose: the first paint waits on a poll of the daemon,
      // and CI runners share a machine with whatever else is building.
      timeout,
      interval: 250,
      timeoutMsg: `fewer than ${atLeast} rows ever rendered`,
    },
  );
}

/** Focus the scroll container that owns the list's key handling. */
export async function focusList(browser: Browser): Promise<void> {
  await browser.execute(function (selector: string) {
    (document.querySelector(selector) as HTMLElement).focus();
  }, HISTORY_LIST);
}

export async function activeRowId(browser: Browser): Promise<string | null> {
  return (await browser.execute(function (selector: string) {
    const el = document.querySelector(selector) as HTMLElement | null;
    return el?.getAttribute("data-active-descendant")?.replace(/^history-row-/, "") ?? null;
  }, HISTORY_LIST)) as string | null;
}

/**
 * Switch screens the way a user does.
 *
 * The landmark is located first and the text selector applied to it: WebDriver
 * has no "CSS then text" syntax, and a combined selector is rejected as one
 * malformed selector rather than treated as two.
 */
export async function gotoView(browser: Browser, label: string): Promise<void> {
  const nav = await browser.$(PRIMARY_NAVIGATION);
  await nav.waitForExist({ timeout: 15_000 });
  const button = await nav.$(`button=${label}`);
  await button.waitForClickable({ timeout: 15_000 });
  await button.click();
  await browser.waitUntil(
    async () => (await button.getAttribute("aria-current")) === "page",
    { timeout: 15_000, timeoutMsg: `the ${label} screen never became current` },
  );
}

export async function visibleText(browser: Browser): Promise<string> {
  return (await browser.execute(() => document.body.innerText)) as string;
}

/**
 * Wait for a phrase to be *rendered*.
 *
 * Against `innerText`, never against a catalogue key: the strings are moving
 * into i18n, and a test that matched a key would keep passing while the screen
 * showed one.
 */
export async function waitForText(
  browser: Browser,
  needle: string,
  timeout = 30_000,
): Promise<void> {
  await browser.waitUntil(
    async () => (await visibleText(browser)).includes(needle),
    {
      timeout,
      interval: 250,
      timeoutMsg: `never rendered ${JSON.stringify(needle)}`,
    },
  );
}

/** Every element carrying this accessible name, with the box it was laid out
 *  at — so "present" and "rendered" are told apart. */
export async function byLabel(
  browser: Browser,
  label: string,
): Promise<Array<{ tag: string; width: number; height: number; text: string }>> {
  return (await browser.execute(function (name: string) {
    return Array.prototype.map.call(
      document.querySelectorAll('[aria-label="' + name + '"]'),
      function (node) {
        const el = node as HTMLElement;
        const rect = el.getBoundingClientRect();
        return {
          tag: el.tagName,
          width: rect.width,
          height: rect.height,
          text: el.innerText,
        };
      },
    );
  }, label)) as Array<{ tag: string; width: number; height: number; text: string }>;
}

/**
 * First launch owns the window until the welcome screen is dismissed. History
 * E2E is the product shell, not the welcome flow.
 */
export async function dismissFirstRun(browser: Browser): Promise<void> {
  await browser.waitUntil(
    async () => browser.execute((primaryNavigation: string) => {
      if (document.querySelector(primaryNavigation)) return true;
      const explore = Array.from(
        document.querySelectorAll<HTMLButtonElement>(
          '[data-onboarding-step="welcome"] button',
        ),
      ).find((button) => button.textContent?.trim() === "Explore first");
      if (!explore) return false;
      // This dismissal is setup for unrelated product tests. Platform drivers
      // disagree on hit-testing the tall welcome grid; tested actions do not.
      explore.click();
      return false;
    }, PRIMARY_NAVIGATION),
    {
      timeout: 30_000,
      timeoutMsg: "the welcome flow never yielded to primary navigation",
    },
  );
}

/**
 * Click a button by the label a user sees — its rendered text, or its
 * accessible name when it has only an icon.
 *
 * A real WebDriver click, not `element.click()` from a script: hit-testing,
 * pointer events and focus are the parts of a control this suite exists to
 * exercise, and a synthetic dispatch skips all three.
 */
export async function clickButton(
  browser: Browser,
  label: string,
  options: { within?: string; timeout?: number } = {},
): Promise<void> {
  const { within, timeout = 15_000 } = options;
  const root = within ? await browser.$(within) : browser;
  const byText = await root.$(`button=${label}`);
  const target = (await byText.isExisting())
    ? byText
    : await root.$(`button[aria-label="${label}"]`);
  await target.waitForClickable({
    timeout,
    timeoutMsg:
      `no clickable button labelled ${JSON.stringify(label)}` +
      (within ? ` inside ${within}` : ""),
  });
  await target.click();
}
