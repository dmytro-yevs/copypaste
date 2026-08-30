import { describe, expect, test } from "vitest";

import { parseImeRuntimeDiagnostics } from "../src/harness/native-ime-diagnostics.js";

const IME_WINDOW = [
  "  mInputMethodWindow=Window{ime u0 com.android.inputmethod/.ImeService}",
  "  Window #6 Window{ime u0 com.android.inputmethod/.ImeService}",
  "    Frames: parent=[0,0][1080,1920] display=[0,0][1080,1920] frame=[0,1320][1080,1920] last=[0,1320][1080,1920]",
  "    isVisible=true",
].join("\n");

describe("Android IME diagnostics", () => {
  test("reads Android 14 input requests without claiming a reported IME visibility", () => {
    expect(
      parseImeRuntimeDiagnostics(
        "  mRequestedShowExplicitly=false mShowForced=false\n  mInputShown=true",
        IME_WINDOW,
      ),
    ).toEqual({
      inputShown: true,
      reportedImeVisible: "unknown",
      imeWindowPresent: true,
      imeWindowVisible: true,
      imeWindowFrame: { left: 0, top: 1320, width: 1080, height: 600 },
    });
  });

  test("reads Android 16's reported visibility flag separately from input shown", () => {
    expect(
      parseImeRuntimeDiagnostics(
        "      mImeWindowVis=3\n      mInputShown=false",
        IME_WINDOW.replace("isVisible=true", "isVisible=false"),
      ),
    ).toEqual({
      inputShown: false,
      reportedImeVisible: true,
      imeWindowPresent: true,
      imeWindowVisible: false,
      imeWindowFrame: { left: 0, top: 1320, width: 1080, height: 600 },
    });
  });

  test("keeps missing and malformed fields unknown without retaining dump text", () => {
    expect(parseImeRuntimeDiagnostics(undefined, "")).toEqual({
      inputShown: "unknown",
      reportedImeVisible: "unknown",
      imeWindowPresent: "unknown",
      imeWindowVisible: "unknown",
      imeWindowFrame: "unknown",
    });
    expect(
      parseImeRuntimeDiagnostics(
        "mInputShown=maybe\nmImeWindowVis=8",
        "mInputMethodWindow=not-a-window",
      ),
    ).toEqual({
      inputShown: "unknown",
      reportedImeVisible: "unknown",
      imeWindowPresent: "unknown",
      imeWindowVisible: "unknown",
      imeWindowFrame: "unknown",
    });
  });

  test("reports false states without treating an IME window object as visible", () => {
    expect(
      parseImeRuntimeDiagnostics(
        "      mImeWindowVis=1\n      mInputShown=false",
        [
          "  mInputMethodWindow=Window{ime u0 com.android.inputmethod/.ImeService}",
          "  Window #6 Window{ime u0 com.android.inputmethod/.ImeService}",
          "    Frames: frame=[0,1920][1080,1920]",
          "    isVisible=false",
        ].join("\n"),
      ),
    ).toEqual({
      inputShown: false,
      reportedImeVisible: false,
      imeWindowPresent: true,
      imeWindowVisible: false,
      imeWindowFrame: { left: 0, top: 1920, width: 1080, height: 0 },
    });
  });
});
