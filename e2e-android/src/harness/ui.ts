import { sleep } from "./adb.js";
import type { AndroidApp } from "./app.js";
import type { Page } from "puppeteer-core";

export const NAV = 'nav[aria-label="Primary"]';
export const NAVIGATION_READY = `${NAV} button:not(:disabled):not([aria-disabled="true"])`;
export const HISTORY_LIST = '[role="list"][aria-label="Clipboard history"]';
export const ROW = `${HISTORY_LIST} [role="listitem"]`;
export const ROW_SELECTION = `${ROW} [role="checkbox"]`;
export const SEARCH_DEFAULT_LABEL = "Search clipboard history, default";
export const SEARCH =
  '[role="searchbox"][aria-label^="Search clipboard history,"]';
export const HISTORY_SEARCH_EXPANDED_ATTRIBUTE = "data-search-expanded";
export const MASKED_ROW =
  '[aria-label="Sensitive item, hidden — activate to reveal"]';

export async function visibleText(app: AndroidApp): Promise<string> {
  return app.withPage((page) => page.evaluate(() => document.body.innerText));
}

export async function count(
  app: AndroidApp,
  selector: string,
): Promise<number> {
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

export function allPrimaryNavigationButtonsReady(
  buttons: ReadonlyArray<{ disabled: boolean; ariaDisabled: string | null }>,
): boolean {
  return (
    buttons.length > 0 &&
    buttons.every(
      (button) => !button.disabled && button.ariaDisabled !== "true",
    )
  );
}

async function navigationIsReady(app: AndroidApp): Promise<boolean> {
  const buttons = await app.withPage((page) =>
    page.evaluate((nav) => {
      const root = document.querySelector(nav);
      return root
        ? Array.from(
            root.querySelectorAll<HTMLButtonElement>("button"),
            (button) => ({
              disabled: button.disabled,
              ariaDisabled: button.getAttribute("aria-disabled"),
            }),
          )
        : [];
    }, NAV),
  );
  return allPrimaryNavigationButtonsReady(buttons);
}

/**
 * First launch owns the window until the welcome flow is dismissed. History
 * E2E is the product shell; onboarding has no primary navigation landmark.
 */
export async function dismissFirstRun(app: AndroidApp): Promise<void> {
  await waitFor(
    async () => {
      if (await navigationIsReady(app)) return true;
      return app.withPage((page) =>
        page.evaluate(() => {
          const explore = Array.from(
            document.querySelectorAll<HTMLButtonElement>(
              '[data-onboarding-step="welcome"] button',
            ),
          ).find((button) => button.textContent?.trim() === "Explore first");
          explore?.click();
          return false;
        }),
      );
    },
    "the welcome flow never yielded to settled navigation",
    60_000,
  );
}

/** Switch screens the way a user does — see `tapWhere` for why the tap is
 *  dispatched at a point this harness computes rather than by `click()`. */
export async function gotoView(app: AndroidApp, label: string): Promise<void> {
  await dismissFirstRun(app);
  await waitFor(
    () => navigationIsReady(app),
    "Android navigation never settled after capture health loaded",
    60_000,
  );
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
        return (
          !!first &&
          (first.matches(masked) || first.querySelector(masked) !== null)
        );
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

export interface ElementBox {
  width: number;
  height: number;
}

interface ElementCandidate extends ElementBox {
  disabled: boolean;
  ariaDisabled: string | null;
  ownsCenter: boolean;
  x: number;
  y: number;
  right: number;
}

interface InteractableElement extends ElementCandidate {
  index: number;
}

export function anyElementRendered(boxes: readonly ElementBox[]): boolean {
  return boxes.some((box) => box.width > 0 && box.height > 0);
}

export function firstInteractableElementIndex(
  candidates: readonly Pick<
    ElementCandidate,
    "width" | "height" | "disabled" | "ariaDisabled" | "ownsCenter"
  >[],
): number {
  return candidates.findIndex(
    (candidate) =>
      candidate.width > 0 &&
      candidate.height > 0 &&
      !candidate.disabled &&
      candidate.ariaDisabled !== "true" &&
      candidate.ownsCenter,
  );
}

async function firstInteractableElement(
  page: Page,
  selector: string,
): Promise<InteractableElement | null> {
  const candidates = await page.evaluate(
    (query) =>
      Array.from(document.querySelectorAll(query), (node) => {
        const target = node as HTMLElement;
        const rect = target.getBoundingClientRect();
        const x = rect.x + rect.width / 2;
        const y = rect.y + rect.height / 2;
        return {
          disabled: target.matches(":disabled"),
          ariaDisabled: target.getAttribute("aria-disabled"),
          width: rect.width,
          height: rect.height,
          x,
          y,
          right: rect.right,
          ownsCenter:
            rect.width > 0 &&
            rect.height > 0 &&
            target.contains(document.elementFromPoint(x, y)),
        };
      }),
    selector,
  );
  const index = firstInteractableElementIndex(candidates);
  return index < 0 ? null : { ...candidates[index], index };
}

async function withInteractableElement<T>(
  app: AndroidApp,
  selector: string,
  action: (page: Page, element: InteractableElement) => Promise<T>,
): Promise<T> {
  return app.withPage(async (page) => {
    const element = await firstInteractableElement(page, selector);
    if (!element)
      throw new Error(`no interactable element matched ${selector}`);
    return action(page, element);
  });
}

export async function interactableElementBox(
  app: AndroidApp,
  selector: string,
): Promise<(ElementBox & { right: number }) | null> {
  return app.withPage(async (page) => {
    const element = await firstInteractableElement(page, selector);
    return element
      ? {
          width: element.width,
          height: element.height,
          right: element.right,
        }
      : null;
  });
}

export async function interactableControlSurfaceBox(
  app: AndroidApp,
  selector: string,
): Promise<(ElementBox & { right: number }) | null> {
  return app.withPage(async (page) => {
    const element = await firstInteractableElement(page, selector);
    if (!element) return null;
    return page.evaluate(
      (query, index) => {
        const target = document.querySelectorAll(query)[index] as
          | HTMLElement
          | undefined;
        const surface = target?.closest<HTMLElement>(
          '[data-slot="control-surface"]',
        );
        if (!surface) return null;
        const rect = surface.getBoundingClientRect();
        return {
          width: rect.width,
          height: rect.height,
          right: rect.right,
        };
      },
      selector,
      element.index,
    );
  });
}

/** Every element carrying this accessible name, with the box it was laid out
 *  at — so "present" and "rendered" are told apart. The query ignores CSS, so a
 *  control hidden with `display: none` is still counted. */
export async function byLabel(
  app: AndroidApp,
  label: string,
): Promise<LabelledBox[]> {
  return app.withPage((page) =>
    page.evaluate(
      (name: string) =>
        Array.from(
          document.querySelectorAll(`[aria-label="${name}"]`),
          (node) => {
            const el = node as HTMLElement;
            const rect = el.getBoundingClientRect();
            return {
              tag: el.tagName,
              width: rect.width,
              height: rect.height,
              text: el.innerText,
            };
          },
        ),
      label,
    ),
  );
}

/**
 * Tap a live-page point only after `elementFromPoint` proves the target owns
 * it. This caught an overflowing settings label covering a neighbouring tab.
 *
 * `ElementHandle.click` intersects quads with stale `Page.getLayoutMetrics`:
 * this inset-resized WebView rejected a visible button against its old height.
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
      (
        root: string | null,
        query: string,
        name: string | null,
        nth: number,
      ) => {
        const within = root ? document.querySelector(root) : document;
        if (!within) return null;
        const matches = Array.from(within.querySelectorAll(query)).filter(
          (node) => {
            if (name === null) return true;
            const el = node as HTMLElement;
            return (
              el.textContent?.trim() === name ||
              el.getAttribute("aria-label") === name
            );
          },
        );
        // A negative index means "the first one a tap can actually reach":
        // the list is virtualised, so its first row in document order may be
        // scrolled under the toolbar while four identical controls below it
        // are on screen.
        const candidates = nth < 0 ? matches : matches.slice(nth, nth + 1);
        for (const node of candidates) {
          const target = node as HTMLElement;
          if (
            target.matches(":disabled") ||
            target.getAttribute("aria-disabled") === "true"
          ) {
            continue;
          }
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

/** Tap the first reachable match, including row-scoped controls without a
 * stable label shared across fixtures. */
export async function tapElement(
  app: AndroidApp,
  selector: string,
  label: string | null = null,
  timeout = 15_000,
): Promise<void> {
  await waitFor(
    () => tapWhere(app, null, selector, label, -1),
    `no tappable ${selector}${label ? ` labelled ${JSON.stringify(label)}` : ""}`,
    timeout,
  );
}

export async function fieldValue(
  app: AndroidApp,
  selector: string,
): Promise<string> {
  return withInteractableElement(app, selector, (page, element) =>
    page.evaluate(
      (query, index) =>
        (document.querySelectorAll(query)[index] as HTMLInputElement).value,
      selector,
      element.index,
    ),
  );
}

/** Tap, then type on the keyboard — the mobile path to a text field, and the
 *  one that proves the WebView takes key input at all. */
export async function typeInto(
  app: AndroidApp,
  selector: string,
  text: string,
): Promise<void> {
  await withInteractableElement(app, selector, async (page, element) => {
    await page.mouse.click(element.x, element.y);
    await page.keyboard.type(text, { delay: 20 });
  });
}

/**
 * Backspace from the end rather than select-all: a triple click selects a word
 * on some engines and the whole value on others, and the difference is a
 * half-cleared filter that the next assertion reads as a missing item.
 */
export async function clearField(
  app: AndroidApp,
  selector: string,
): Promise<void> {
  const current = await fieldValue(app, selector);
  if (!current) return;
  await withInteractableElement(app, selector, async (page, element) => {
    await page.mouse.click(element.x, element.y);
    await page.keyboard.press("End");
    for (let i = 0; i < current.length; i++)
      await page.keyboard.press("Backspace");
  });
  await waitFor(
    async () => (await fieldValue(app, selector)) === "",
    `${selector} still holds text after clearing it`,
  );
}

/** Restore the toolbar state a shared device may retain between files or runs. */
export async function resetHistoryFilters(app: AndroidApp): Promise<void> {
  const kind = 'button[aria-label^="Filter by kind,"]';
  if ((await count(app, `${kind}[data-active-filter]`)) > 0) {
    await tapElement(app, kind);
    await tapElement(app, '[role="menuitemcheckbox"]', "All kinds");
  }
  const sort = '[role="combobox"][aria-label^="Sort order,"]';
  if ((await count(app, `${sort}[data-active-filter]`)) > 0) {
    await tapElement(app, sort);
    await tapElement(app, '[role="option"][data-value="newest"]');
  }
  await openHistorySearch(app);
  await clearField(app, SEARCH);
  if ((await byLabel(app, "Close search")).length > 0) {
    await tapButton(app, "Close search");
  }
}

export async function closeHistorySearch(app: AndroidApp): Promise<void> {
  const expanded = '[data-slot="history-toolbar"][data-search-expanded]';
  if ((await count(app, expanded)) === 0) return;
  await clearField(app, SEARCH);
  await tapButton(app, "Close search");
  await waitFor(
    async () => (await count(app, expanded)) === 0,
    "the expanded history search never closed",
  );
}

export async function openHistorySearch(app: AndroidApp): Promise<void> {
  if (await interactableElementBox(app, SEARCH)) return;
  await tapElement(app, 'button[aria-label^="Search clipboard history,"]');
  await waitFor(
    async () => (await interactableElementBox(app, SEARCH)) !== null,
    "the search field never opened",
  );
}

/**
 * Isolate a suite's rows through the product search path. A unique query reads
 * the store directly and cannot inherit stale pages from another suite.
 */
export async function filterHistoryTo(
  app: AndroidApp,
  query: string,
  expectedText: string,
): Promise<void> {
  await resetHistoryFilters(app);
  await openHistorySearch(app);
  await typeInto(app, SEARCH, query);
  await waitFor(
    async () =>
      (await fieldValue(app, SEARCH)) === query &&
      (await visibleText(app)).includes(expectedText),
    `history search never rendered ${JSON.stringify(expectedText)}`,
    60_000,
  );
  await scrollListToTop(app);
}

/** Start the history view with a new query cache after seeding through the bridge. */
export async function reloadHistoryWith(
  app: AndroidApp,
  expectedText: string,
): Promise<void> {
  await app.withPage(async (page) => {
    await page.reload({ waitUntil: "domcontentloaded" });
  });
  await waitFor(
    async () => (await count(app, `${NAV} button`)) > 0,
    "the WebView never mounted after reload",
    60_000,
  );
  await gotoView(app, "Library");
  await resetHistoryFilters(app);
  await scrollListToTop(app);
  await waitFor(
    async () => (await visibleText(app)).includes(expectedText),
    `fresh history query never rendered ${JSON.stringify(expectedText)}`,
    60_000,
  );
}
