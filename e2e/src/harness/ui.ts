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

export async function rowCount(browser: Browser): Promise<number> {
  return (await browser.execute(
    (selector: string) => document.querySelectorAll(selector).length,
    ROW,
  )) as number;
}

export async function waitForRows(browser: Browser, atLeast = 1): Promise<void> {
  await browser.waitUntil(
    async () => (await rowCount(browser)) >= atLeast,
    {
      timeout: 30_000,
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

export async function gotoView(browser: Browser, label: string): Promise<void> {
  const button = await browser.$(`nav[aria-label="Primary"] button=${label}`);
  await button.waitForClickable({ timeout: 15_000 });
  await button.click();
}

export async function visibleText(browser: Browser): Promise<string> {
  return (await browser.execute(() => document.body.innerText)) as string;
}
