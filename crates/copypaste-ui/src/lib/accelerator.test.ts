/**
 * Manifest §7.3 capture rules: AT-56 (layout independence), AT-57 (bare
 * modifiers and modifier-less keys), AT-58 (Escape cancels), plus the media-key
 * refusal ADR-0001 requires.
 */
import { describe, expect, it } from "vitest";

import {
  DEFAULT_SHORTCUT,
  MEDIA_KEYS,
  acceleratorGlyphs,
  captureAccelerator,
} from "./accelerator";

function press(over: Partial<KeyboardEvent>) {
  return captureAccelerator({
    code: "",
    key: "",
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    ...over,
  } as KeyboardEvent);
}

describe("layout independence (AT-56)", () => {
  it("reads the physical key, so a Cyrillic layout binds the same combination", () => {
    // The user presses the key engraved Q; the layout reports "й".
    const result = press({ code: "KeyQ", key: "й", metaKey: true, shiftKey: true });
    expect(result).toEqual({ kind: "accelerator", value: "CmdOrCtrl+Shift+Q" });
  });

  it("strips the Digit prefix", () => {
    expect(press({ code: "Digit1", key: "1", ctrlKey: true })).toEqual({
      kind: "accelerator",
      value: "CmdOrCtrl+1",
    });
  });

  it("keeps a function key whole", () => {
    expect(press({ code: "F5", key: "F5", altKey: true })).toEqual({
      kind: "accelerator",
      value: "Alt+F5",
    });
  });

  it("uses Tauri's spelling for the named keys", () => {
    expect(press({ code: "ArrowUp", key: "ArrowUp", metaKey: true })).toEqual({
      kind: "accelerator",
      value: "CmdOrCtrl+Up",
    });
    expect(press({ code: "Enter", key: "Enter", metaKey: true })).toEqual({
      kind: "accelerator",
      value: "CmdOrCtrl+Return",
    });
  });
});

describe("what is not a shortcut (AT-57)", () => {
  it.each(["Meta", "Control", "Alt", "Shift"])(
    "ignores a bare %s keydown",
    (key) => {
      expect(press({ code: key, key }).kind).toBe("incomplete");
    },
  );

  it("refuses a key with no modifier, and says what is missing", () => {
    const result = press({ code: "KeyV", key: "v" });
    expect(result.kind).toBe("refused");
    if (result.kind === "refused") expect(result.reason).toMatch(/modifier/i);
  });
});

describe("Escape cancels (AT-58)", () => {
  it("cancels rather than binding", () => {
    expect(press({ code: "Escape", key: "Escape" }).kind).toBe("cancelled");
  });

  it("still binds Escape when it is part of a combination", () => {
    expect(press({ code: "Escape", key: "Escape", metaKey: true })).toEqual({
      kind: "accelerator",
      value: "CmdOrCtrl+Escape",
    });
  });
});

describe("media keys are refused", () => {
  it.each(MEDIA_KEYS)("%s is refused with a reason the user can act on", (code) => {
    // ADR-0001: binding one takes a CGEventTap, which needs Accessibility, and
    // an ad-hoc-signed app loses that grant on every update — the shortcut
    // would stop working after an upgrade. The bridge refuses these too.
    const result = press({ code, key: code, metaKey: true });
    expect(result.kind).toBe("refused");
    if (result.kind === "refused") {
      expect(result.reason).toMatch(/Accessibility/);
      expect(result.reason).toMatch(/update/);
    }
  });
});

describe("modifier order is fixed", () => {
  it("is always CmdOrCtrl, Alt, Shift, key", () => {
    expect(
      press({
        code: "KeyV",
        key: "v",
        shiftKey: true,
        altKey: true,
        ctrlKey: true,
      }),
    ).toEqual({ kind: "accelerator", value: "CmdOrCtrl+Alt+Shift+V" });
  });

  it("maps both Meta and Ctrl onto CmdOrCtrl", () => {
    expect(press({ code: "KeyV", key: "v", metaKey: true })).toEqual(
      press({ code: "KeyV", key: "v", ctrlKey: true }),
    );
  });
});

describe("display is separate from the value (A11Y-13)", () => {
  it("renders one keycap per token, so F1 does not become two", () => {
    expect(acceleratorGlyphs("CmdOrCtrl+Shift+F1")).toEqual(["⌘", "⇧", "F1"]);
  });

  it("passes an unmapped token through rather than dropping it", () => {
    expect(acceleratorGlyphs("CmdOrCtrl+Q")).toEqual(["⌘", "Q"]);
  });

  it("has a default that matches the one the backend registers", () => {
    expect(DEFAULT_SHORTCUT).toBe("CmdOrCtrl+Shift+V");
  });
});
