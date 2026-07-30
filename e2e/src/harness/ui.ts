import type { Browser } from "./app.js";

export const HISTORY_LIST = '[role="list"][aria-label="Clipboard history"]';
export const ROW = '[role="listitem"]';

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
  return (await browser.execute(function (selector: string) {
    return Array.prototype.map.call(
      document.querySelectorAll(selector),
      function (node) {
        const el = node as HTMLElement;
        const rect = el.getBoundingClientRect();
        const match = /translateY\(([-0-9.]+)px\)/.exec(el.style.transform);
        return {
          id: el.id.replace(/^history-row-/, ""),
          start: match && match[1] !== undefined ? parseFloat(match[1]) : NaN,
          height: rect.height,
          active: el.getAttribute("aria-current") === "true",
          text: el.innerText,
        };
      },
    );
  }, ROW)) as RowBox[];
}

export async function scroller(browser: Browser): Promise<{
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  totalSize: number;
}> {
  return (await browser.execute(function (selector: string) {
    const list = document.querySelector(selector) as HTMLElement | null;
    const el = list?.parentElement as HTMLElement;
    return {
      scrollTop: el.scrollTop,
      scrollHeight: el.scrollHeight,
      clientHeight: el.clientHeight,
      totalSize: list ? list.getBoundingClientRect().height : NaN,
    };
  }, HISTORY_LIST)) as {
    scrollTop: number;
    scrollHeight: number;
    clientHeight: number;
    totalSize: number;
  };
}

export async function scrollTo(browser: Browser, top: number): Promise<void> {
  await browser.execute(
    function (selector: string, offset: number) {
      const list = document.querySelector(selector) as HTMLElement | null;
      const el = list?.parentElement as HTMLElement;
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
    const list = document.querySelector(selector) as HTMLElement | null;
    (list?.parentElement as HTMLElement).focus();
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
 * The nav is located first and the text selector applied to it: WebDriver has
 * no "CSS then text" syntax, and `nav[…] button=History` is rejected as one
 * malformed selector rather than treated as two.
 */
export async function gotoView(browser: Browser, label: string): Promise<void> {
  const nav = await browser.$('nav[aria-label="Primary"]');
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
