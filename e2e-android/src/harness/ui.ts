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

export async function waitFor(
  predicate: () => Promise<boolean>,
  message: string,
  timeout = 30_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await sleep(250);
  }
  throw new Error(message);
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

/**
 * Switch screens the way a user does, with a real tap: `ElementHandle.click`
 * scrolls the node into view, hit-tests its box and dispatches through
 * `Input.dispatchMouseEvent`, so a control the layout has covered fails here.
 * A script-side `element.click()` would skip all three.
 */
export async function gotoView(app: AndroidApp, label: string): Promise<void> {
  await app.withPage(async (page) => {
    for (const handle of await page.$$(`${NAV} button`)) {
      if ((await handle.evaluate((node) => node.textContent?.trim())) !== label) continue;
      await handle.click();
      await waitFor(
        async () =>
          (await handle.evaluate((node) => node.getAttribute("aria-current"))) === "page",
        `the ${label} screen never became current`,
      );
      return;
    }
    throw new Error(`no button labelled ${JSON.stringify(label)} inside ${NAV}`);
  });
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
        return !!first && (first.matches(masked) || first.querySelector(masked) !== null);
      },
      ROW,
      MASKED_ROW,
    ),
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
