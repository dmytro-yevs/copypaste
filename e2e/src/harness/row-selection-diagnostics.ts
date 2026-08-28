import type { Browser } from "./webview-guard.js";
import { selectionDiagnosticFailure } from "./selection-diagnostics.js";

interface RowEventReceipt {
  pointerDown: number;
  pointerUp: number;
  click: number;
  trusted: {
    pointerDown: boolean[];
    pointerUp: boolean[];
    click: boolean[];
  };
}

interface PageRowProbe {
  id: string;
  checkbox: HTMLElement;
  receipt: RowEventReceipt;
  handlers: {
    pointerDown: EventListener;
    pointerUp: EventListener;
    click: EventListener;
  };
}

interface RowProbeWindow extends Window {
  __copypasteRowSelectionClickProbe?: PageRowProbe;
}

export interface RowSelectionSnapshot {
  id: string;
  checkboxChecked: boolean;
  checkboxDisplayed: boolean;
  checkboxClickable: boolean;
  checkedIds: readonly string[];
  renderedPinnedBadgeIds: readonly string[];
  toolbarPinToggleLabel: "Pin" | "Unpin" | null;
  events: RowEventReceipt;
}

export type RowSelectionProbeRead =
  | { available: true; snapshot: RowSelectionSnapshot }
  | { available: false; reason: "read-failed" };

export interface RowSelectionClickReceipt {
  id: string;
  before: RowSelectionProbeRead;
  after: RowSelectionProbeRead;
}

function installPageRowSelectionClickProbe(id: string): number {
  const probeWindow = window as RowProbeWindow;
  const previous = probeWindow.__copypasteRowSelectionClickProbe;
  if (previous) {
    previous.checkbox.removeEventListener(
      "pointerdown",
      previous.handlers.pointerDown,
      true,
    );
    previous.checkbox.removeEventListener(
      "pointerup",
      previous.handlers.pointerUp,
      true,
    );
    previous.checkbox.removeEventListener("click", previous.handlers.click, true);
    delete probeWindow.__copypasteRowSelectionClickProbe;
  }
  const row = Array.from(
    document.querySelectorAll<HTMLElement>(
      '[role="listitem"][id^="history-row-"]',
    ),
  ).find((candidate) => candidate.id === `history-row-${id}`);
  const matches = row
    ? Array.from(row.querySelectorAll<HTMLElement>('[role="checkbox"]'))
    : [];
  if (matches.length !== 1) return matches.length;

  const checkbox = matches[0]!;
  const receipt: RowEventReceipt = {
    pointerDown: 0,
    pointerUp: 0,
    click: 0,
    trusted: { pointerDown: [], pointerUp: [], click: [] },
  };
  const record = (kind: keyof Omit<RowEventReceipt, "trusted">) =>
    (event: Event) => {
      receipt[kind] += 1;
      receipt.trusted[kind].push(event.isTrusted);
    };
  const handlers = {
    pointerDown: record("pointerDown"),
    pointerUp: record("pointerUp"),
    click: record("click"),
  };
  checkbox.addEventListener("pointerdown", handlers.pointerDown, true);
  checkbox.addEventListener("pointerup", handlers.pointerUp, true);
  checkbox.addEventListener("click", handlers.click, true);
  probeWindow.__copypasteRowSelectionClickProbe = {
    id,
    checkbox,
    receipt,
    handlers,
  };
  return matches.length;
}

function readPageRowSelectionClickProbe(): RowSelectionSnapshot {
  const probe = (window as RowProbeWindow).__copypasteRowSelectionClickProbe;
  if (!probe) throw new Error("row selection probe was not armed");
  const rect = probe.checkbox.getBoundingClientRect();
  const style = getComputedStyle(probe.checkbox);
  const displayed =
    probe.checkbox.isConnected &&
    rect.width > 0 &&
    rect.height > 0 &&
    style.display !== "none" &&
    style.visibility !== "hidden" &&
    style.opacity !== "0";
  const disabled = probe.checkbox.getAttribute("aria-disabled") === "true";
  const center = displayed
    ? document.elementFromPoint(
        rect.left + rect.width / 2,
        rect.top + rect.height / 2,
      )
    : null;
  const toolbar = Array.from(
    document.querySelectorAll<HTMLElement>(
      '[role="toolbar"][aria-label="Selection actions"]',
    ),
  ).find((candidate) => {
    const box = candidate.getBoundingClientRect();
    return box.width > 0 && box.height > 0;
  });
  const toolbarPinToggleLabel = toolbar
    ? Array.from(toolbar.querySelectorAll<HTMLButtonElement>("button"))
        .map((button) => button.getAttribute("aria-label"))
        .find((label): label is "Pin" | "Unpin" =>
          label === "Pin" || label === "Unpin",
        ) ?? null
    : null;

  return {
    id: probe.id,
    checkboxChecked: probe.checkbox.getAttribute("aria-checked") === "true",
    checkboxDisplayed: displayed,
    checkboxClickable:
      displayed &&
      !disabled &&
      style.pointerEvents !== "none" &&
      (center === probe.checkbox || probe.checkbox.contains(center)),
    checkedIds: Array.from(
      document.querySelectorAll<HTMLElement>(
        '[role="listitem"][id^="history-row-"][aria-checked="true"]',
      ),
      (row) => row.id.replace(/^history-row-/, ""),
    ),
    renderedPinnedBadgeIds: Array.from(
      document.querySelectorAll<HTMLElement>(
        '[role="listitem"][id^="history-row-"]',
      ),
    )
      .filter((row) => row.querySelector('[title="Pinned"]') !== null)
      .map((row) => row.id.replace(/^history-row-/, "")),
    toolbarPinToggleLabel,
    events: {
      pointerDown: probe.receipt.pointerDown,
      pointerUp: probe.receipt.pointerUp,
      click: probe.receipt.click,
      trusted: {
        pointerDown: [...probe.receipt.trusted.pointerDown],
        pointerUp: [...probe.receipt.trusted.pointerUp],
        click: [...probe.receipt.trusted.click],
      },
    },
  };
}

function disarmPageRowSelectionClickProbe(): boolean {
  const probeWindow = window as RowProbeWindow;
  const probe = probeWindow.__copypasteRowSelectionClickProbe;
  if (!probe) return false;
  probe.checkbox.removeEventListener("pointerdown", probe.handlers.pointerDown, true);
  probe.checkbox.removeEventListener("pointerup", probe.handlers.pointerUp, true);
  probe.checkbox.removeEventListener("click", probe.handlers.click, true);
  delete probeWindow.__copypasteRowSelectionClickProbe;
  return true;
}

async function readRowSelectionProbe(browser: Browser): Promise<RowSelectionProbeRead> {
  try {
    return {
      available: true,
      snapshot: (await browser.execute(
        readPageRowSelectionClickProbe,
      )) as RowSelectionSnapshot,
    };
  } catch {
    return { available: false, reason: "read-failed" };
  }
}

async function disarmRowSelectionProbe(browser: Browser): Promise<void> {
  try {
    await browser.execute(disarmPageRowSelectionClickProbe);
  } catch {
    // Cleanup cannot replace the observed click failure.
  }
}

export async function captureRowSelectionClick(
  browser: Browser,
  id: string,
  click: () => Promise<void>,
  failureDetail: Record<string, unknown>,
): Promise<RowSelectionClickReceipt> {
  const matchCount = (await browser.execute(
    installPageRowSelectionClickProbe,
    id,
  )) as number;
  if (matchCount !== 1) {
    throw selectionDiagnosticFailure(
      new Error("row selection control unavailable"),
      "row-selection-probe-arm",
      { ...failureDetail, id, matchCount },
    );
  }

  const before = await readRowSelectionProbe(browser);
  let after: RowSelectionProbeRead = { available: false, reason: "read-failed" };
  try {
    await click();
    after = await readRowSelectionProbe(browser);
    return { id, before, after };
  } catch (cause) {
    after = await readRowSelectionProbe(browser);
    throw selectionDiagnosticFailure(cause, "row-selection-click", {
      ...failureDetail,
      receipt: { id, before, after },
    });
  } finally {
    await disarmRowSelectionProbe(browser);
  }
}
