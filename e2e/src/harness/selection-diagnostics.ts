import type { Browser } from "./webview-guard.js";

export interface SelectionActionSnapshot {
  toolbar: {
    present: boolean;
    displayed: boolean;
    busy: boolean;
    disabled: boolean;
    pinToggleLabel: "Pin" | "Unpin" | null;
    controlCount: number;
    disabledControlCount: number;
  };
  action: {
    label: string;
    connected: boolean;
    displayed: boolean;
    clickable: boolean;
    busy: boolean;
    disabled: boolean;
    rect: { left: number; top: number; width: number; height: number };
    centerTarget: "self" | "descendant" | "toolbar" | "other" | "none";
  };
  events: {
    pointerDown: number;
    pointerUp: number;
    click: number;
    trusted: {
      pointerDown: readonly boolean[];
      pointerUp: readonly boolean[];
      click: readonly boolean[];
    };
  };
  checkedIds: readonly string[];
  renderedPinnedBadgeIds: readonly string[];
  toasts: ReadonlyArray<{
    kind: string;
    name: string;
  }>;
}

interface PageProbeReceipt {
  pointerDown: number;
  pointerUp: number;
  click: number;
  trusted: {
    pointerDown: boolean[];
    pointerUp: boolean[];
    click: boolean[];
  };
}

interface PageProbe {
  action: HTMLButtonElement;
  toolbar: HTMLElement;
  label: string;
  receipt: PageProbeReceipt;
  handlers: {
    pointerDown: EventListener;
    pointerUp: EventListener;
    click: EventListener;
  };
}

interface ProbeWindow extends Window {
  __copypasteSelectionActionProbe?: PageProbe;
}

export async function armSelectionActionProbe(
  browser: Browser,
  label: string,
): Promise<SelectionActionSnapshot> {
  const matchCount = (await browser.execute(function (
    toolbarSelector: string,
    actionLabel: string,
  ) {
    const probeWindow = window as ProbeWindow;
    const previous = probeWindow.__copypasteSelectionActionProbe;
    if (previous) {
      previous.action.removeEventListener(
        "pointerdown",
        previous.handlers.pointerDown,
        true,
      );
      previous.action.removeEventListener(
        "pointerup",
        previous.handlers.pointerUp,
        true,
      );
      previous.action.removeEventListener("click", previous.handlers.click, true);
      delete probeWindow.__copypasteSelectionActionProbe;
    }
    const displayed = (element: HTMLElement) => {
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== "none" &&
        style.visibility !== "hidden"
      );
    };
    const toolbars = Array.from(
      document.querySelectorAll<HTMLElement>(toolbarSelector),
    ).filter(displayed);
    const matches = toolbars.flatMap((toolbar) =>
      Array.from(toolbar.querySelectorAll<HTMLButtonElement>("button")).filter(
        (button) =>
          displayed(button) &&
          (button.getAttribute("aria-label") === actionLabel ||
            button.innerText.trim() === actionLabel),
      ),
    );
    if (matches.length !== 1) return matches.length;

    const action = matches[0]!;
    const toolbar = action.closest<HTMLElement>(toolbarSelector)!;
    const receipt: PageProbeReceipt = {
      pointerDown: 0,
      pointerUp: 0,
      click: 0,
      trusted: { pointerDown: [], pointerUp: [], click: [] },
    };
    const record = (kind: keyof Omit<PageProbeReceipt, "trusted">) =>
      (event: Event) => {
        receipt[kind] += 1;
        receipt.trusted[kind].push(event.isTrusted);
      };
    const handlers = {
      pointerDown: record("pointerDown"),
      pointerUp: record("pointerUp"),
      click: record("click"),
    };
    action.addEventListener("pointerdown", handlers.pointerDown, true);
    action.addEventListener("pointerup", handlers.pointerUp, true);
    action.addEventListener("click", handlers.click, true);
    probeWindow.__copypasteSelectionActionProbe = {
      action,
      toolbar,
      label: actionLabel,
      receipt,
      handlers,
    };
    return matches.length;
  }, '[role="toolbar"][aria-label="Selection actions"]', label)) as number;

  if (matchCount !== 1) {
    throw new Error(
      "selection action probe could not arm; diagnostics=" +
        selectionDiagnosticJson("probe-arm", { label, matchCount }),
    );
  }
  try {
    return await readSelectionActionProbe(browser);
  } catch (cause) {
    await bestEffortDisarmSelectionActionProbe(browser);
    throw selectionDiagnosticFailure(cause, "probe-initial-read", { label });
  }
}

export async function readSelectionActionProbe(
  browser: Browser,
): Promise<SelectionActionSnapshot> {
  return (await browser.execute(function () {
    const probe = (window as ProbeWindow).__copypasteSelectionActionProbe;
    if (!probe) throw new Error("selection action probe was not armed");

    const displayed = (element: HTMLElement) => {
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return (
        element.isConnected &&
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== "none" &&
        style.visibility !== "hidden" &&
        style.opacity !== "0"
      );
    };
    const disabled = (element: HTMLElement) =>
      (element instanceof HTMLButtonElement && element.disabled) ||
      element.getAttribute("aria-disabled") === "true";
    const rect = probe.action.getBoundingClientRect();
    const center =
      rect.width > 0 && rect.height > 0
        ? document.elementFromPoint(
            rect.left + rect.width / 2,
            rect.top + rect.height / 2,
          )
        : null;
    const centerTarget = center === null
      ? "none"
      : center === probe.action
        ? "self"
        : probe.action.contains(center)
          ? "descendant"
          : probe.toolbar.contains(center)
            ? "toolbar"
            : "other";
    const controls = Array.from(
      probe.toolbar.querySelectorAll<HTMLElement>("button, [role=checkbox]"),
    );
    const currentToolbar = Array.from(
      document.querySelectorAll<HTMLElement>(
        '[role="toolbar"][aria-label="Selection actions"]',
      ),
    ).find(displayed);
    const pinToggleLabel = currentToolbar
      ? Array.from(currentToolbar.querySelectorAll<HTMLButtonElement>("button"))
          .map((button) => button.getAttribute("aria-label"))
          .find((label): label is "Pin" | "Unpin" =>
            label === "Pin" || label === "Unpin",
          ) ?? null
      : null;
    const actionDisplayed = displayed(probe.action);
    const actionDisabled = disabled(probe.action);
    const pointerEnabled = getComputedStyle(probe.action).pointerEvents !== "none";
    const clickable =
      actionDisplayed &&
      !actionDisabled &&
      pointerEnabled &&
      (centerTarget === "self" || centerTarget === "descendant");
    const safeToastName = (toast: HTMLElement) => {
      const title = toast.querySelector<HTMLElement>("[data-title]");
      const name = title?.innerText.trim() ?? "";
      return /^(Pinned|Unpinned) \d+(?: item| items| of \d+ — \d+ failed)$/.test(
        name,
      )
        ? name
        : "unclassified";
    };

    return {
      toolbar: {
        present: probe.toolbar.isConnected,
        displayed: displayed(probe.toolbar),
        busy: probe.toolbar.getAttribute("aria-busy") === "true",
        disabled:
          disabled(probe.toolbar) ||
          (controls.length > 0 && controls.every(disabled)),
        pinToggleLabel,
        controlCount: controls.length,
        disabledControlCount: controls.filter(disabled).length,
      },
      action: {
        label: probe.label,
        connected: probe.action.isConnected,
        displayed: actionDisplayed,
        clickable,
        busy: probe.action.getAttribute("aria-busy") === "true",
        disabled: actionDisabled,
        rect: {
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
        },
        centerTarget,
      },
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
      checkedIds: Array.from(
        document.querySelectorAll<HTMLElement>(
          '[role="listitem"][id^="history-row-"] [role="checkbox"][aria-checked="true"]',
        ),
        (checkbox) =>
          checkbox
            .closest<HTMLElement>('[role="listitem"][id^="history-row-"]')
            ?.id.replace(/^history-row-/, ""),
      ).filter((id): id is string => id !== undefined),
      renderedPinnedBadgeIds: Array.from(
        document.querySelectorAll<HTMLElement>(
          '[role="listitem"][id^="history-row-"]',
        ),
      )
        .filter((row) => row.querySelector('[title="Pinned"]') !== null)
        .map((row) => row.id.replace(/^history-row-/, "")),
      toasts: Array.from(
        document.querySelectorAll<HTMLElement>("[data-sonner-toast]"),
        (toast) => ({
          kind: toast.getAttribute("data-type") ?? "unknown",
          name: safeToastName(toast),
        }),
      ),
    };
  })) as SelectionActionSnapshot;
}

export function disarmPageSelectionActionProbe(): boolean {
  const probeWindow = window as ProbeWindow;
  const probe = probeWindow.__copypasteSelectionActionProbe;
  if (!probe) return false;
  probe.action.removeEventListener("pointerdown", probe.handlers.pointerDown, true);
  probe.action.removeEventListener("pointerup", probe.handlers.pointerUp, true);
  probe.action.removeEventListener("click", probe.handlers.click, true);
  delete probeWindow.__copypasteSelectionActionProbe;
  return true;
}

export async function disarmSelectionActionProbe(browser: Browser): Promise<boolean> {
  return (await browser.execute(disarmPageSelectionActionProbe)) as boolean;
}

export type SelectionProbeRead =
  | { available: true; snapshot: SelectionActionSnapshot }
  | { available: false; reason: "not-read" | "read-failed" };

const NOT_READ: SelectionProbeRead = { available: false, reason: "not-read" };

export class SelectionActionProbeSession {
  private closed = false;

  constructor(
    readonly before: SelectionActionSnapshot,
    private readonly readRaw: () => Promise<SelectionActionSnapshot>,
    private readonly disarmRaw: () => Promise<unknown>,
  ) {}

  async read(): Promise<SelectionProbeRead> {
    try {
      return { available: true, snapshot: await this.readRaw() };
    } catch {
      return { available: false, reason: "read-failed" };
    }
  }

  async perform<T>(
    stage: string,
    detail: Record<string, unknown>,
    action: () => Promise<void>,
    wait: () => Promise<T>,
  ): Promise<T> {
    let afterAction = NOT_READ;
    try {
      await action();
      afterAction = await this.read();
      return await wait();
    } catch (cause) {
      throw selectionDiagnosticFailure(cause, stage, {
        ...detail,
        before: this.before,
        afterAction,
        final: await this.read(),
      });
    }
  }

  async failure(
    cause: unknown,
    stage: string,
    detail: Record<string, unknown>,
  ): Promise<Error> {
    return selectionDiagnosticFailure(cause, stage, {
      ...detail,
      before: this.before,
      final: await this.read(),
    });
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    try {
      await this.disarmRaw();
    } catch {
      // Diagnostic cleanup cannot replace the product/test failure.
    }
  }
}

export async function withSelectionActionProbe<T>(
  browser: Browser,
  label: string,
  run: (probe: SelectionActionProbeSession) => Promise<T>,
): Promise<T> {
  const before = await armSelectionActionProbe(browser, label);
  const probe = new SelectionActionProbeSession(
    before,
    () => readSelectionActionProbe(browser),
    () => disarmSelectionActionProbe(browser),
  );
  return runSelectionActionProbeSession(probe, run);
}

export async function runSelectionActionProbeSession<T>(
  probe: SelectionActionProbeSession,
  run: (probe: SelectionActionProbeSession) => Promise<T>,
): Promise<T> {
  try {
    return await run(probe);
  } finally {
    await probe.close();
  }
}

async function bestEffortDisarmSelectionActionProbe(browser: Browser): Promise<void> {
  try {
    await disarmSelectionActionProbe(browser);
  } catch {
    // The initial read failure remains the cause.
  }
}

function redact(value: unknown, key = ""): unknown {
  if (/content|clipboard|path|secret|token|preview|body|value/i.test(key)) {
    return "[redacted]";
  }
  if (Array.isArray(value)) return value.map((entry) => redact(entry));
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([childKey, child]) => [
        childKey,
        redact(child, childKey),
      ]),
    );
  }
  if (
    typeof value === "string" &&
    (/(?:^|\s)(?:\/[\w.-]+){2,}/.test(value) ||
      /[A-Za-z]:\\/.test(value) ||
      /(?:gh[opsu]_|xox[baprs]-|sk-[A-Za-z0-9])/i.test(value))
  ) {
    return "[redacted]";
  }
  return value;
}

export function selectionDiagnosticJson(
  stage: string,
  detail: Record<string, unknown>,
): string {
  return JSON.stringify(redact({ stage, ...detail }));
}

export function selectionDiagnosticFailure(
  cause: unknown,
  stage: string,
  detail: Record<string, unknown>,
): Error {
  return new Error(
    `selection boundary failed; diagnostics=${selectionDiagnosticJson(stage, detail)}`,
    { cause },
  );
}
