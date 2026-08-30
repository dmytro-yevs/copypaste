import { describe, expect, test } from "vitest";

import {
  disarmPageSelectionActionProbe,
  runSelectionActionProbeSession,
  SelectionActionProbeSession,
  selectionDiagnosticJson,
  type SelectionActionSnapshot,
} from "./selection-diagnostics.js";

const SNAPSHOT: SelectionActionSnapshot = {
  toolbar: {
    present: true,
    displayed: true,
    busy: false,
    disabled: false,
    pinToggleLabel: "Pin",
    controlCount: 4,
    disabledControlCount: 0,
  },
  action: {
    label: "Pin",
    connected: true,
    displayed: true,
    clickable: true,
    busy: false,
    disabled: false,
    rect: { left: 1, top: 2, width: 44, height: 44 },
    centerTarget: "self",
  },
  events: {
    pointerDown: 0,
    pointerUp: 0,
    click: 0,
    trusted: { pointerDown: [], pointerUp: [], click: [] },
  },
  checkedIds: ["item-1"],
  renderedPinnedBadgeIds: [],
  toasts: [],
};

describe("selection diagnostics", () => {
  test("keeps boundary facts and removes unsafe values", () => {
    const json = selectionDiagnosticJson("done", {
      checkedIds: ["item-1"],
      eventCount: 1,
      clipboardContent: "private copied text",
      rawPath: "/Users/person/private/database.sqlite",
      nested: {
        secret: "redaction sentinel",
        label: "Done",
      },
    });

    expect(JSON.parse(json)).toEqual({
      stage: "done",
      checkedIds: ["item-1"],
      eventCount: 1,
      clipboardContent: "[redacted]",
      rawPath: "[redacted]",
      nested: {
        secret: "[redacted]",
        label: "Done",
      },
    });
    expect(json).not.toContain("private");
    expect(json).not.toContain("/Users/");
  });

  test("preserves a click failure and cleans the probe once", async () => {
    const clickFailure = new Error("native click failed");
    let cleanupCalls = 0;
    const probe = new SelectionActionProbeSession(
      SNAPSHOT,
      async () => SNAPSHOT,
      async () => {
        cleanupCalls += 1;
      },
    );

    let thrown: unknown;
    try {
      await runSelectionActionProbeSession(probe, (session) =>
        session.perform(
          "pin",
          { budgetMs: 10_000 },
          async () => {
            throw clickFailure;
          },
          async () => undefined,
        ),
      );
    } catch (cause) {
      thrown = cause;
    }

    expect(thrown).toBeInstanceOf(Error);
    expect((thrown as Error).cause).toBe(clickFailure);
    expect((thrown as Error).message).toContain('"reason":"not-read"');
    expect(cleanupCalls).toBe(1);
    await probe.close();
    expect(cleanupCalls).toBe(1);
  });

  test("a failed probe read cannot replace the wait failure", async () => {
    const waitFailure = new Error("selection stayed active");
    const probe = new SelectionActionProbeSession(
      SNAPSHOT,
      async () => {
        throw new Error("document unavailable");
      },
      async () => undefined,
    );

    let thrown: unknown;
    try {
      await probe.perform(
        "done",
        {},
        async () => undefined,
        async () => {
          throw waitFailure;
        },
      );
    } catch (cause) {
      thrown = cause;
    }

    expect(thrown).toBeInstanceOf(Error);
    expect((thrown as Error).cause).toBe(waitFailure);
    expect((thrown as Error).message).toContain('"reason":"read-failed"');
    expect((thrown as Error).message).not.toContain("document unavailable");
  });

  test("page cleanup removes listeners and is idempotent", () => {
    const removals: string[] = [];
    const action = {
      removeEventListener: (type: string) => removals.push(type),
    };
    const probeWindow = {
      __copypasteSelectionActionProbe: {
        action,
        toolbar: {},
        label: "Done",
        receipt: {},
        handlers: {
          pointerDown: () => undefined,
          pointerUp: () => undefined,
          click: () => undefined,
        },
      },
    };
    const original = Object.getOwnPropertyDescriptor(globalThis, "window");
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: probeWindow,
    });
    try {
      expect(disarmPageSelectionActionProbe()).toBe(true);
      expect(disarmPageSelectionActionProbe()).toBe(false);
      expect(removals).toEqual(["pointerdown", "pointerup", "click"]);
      expect(probeWindow.__copypasteSelectionActionProbe).toBeUndefined();
    } finally {
      if (original) Object.defineProperty(globalThis, "window", original);
      else Reflect.deleteProperty(globalThis, "window");
    }
  });
});
