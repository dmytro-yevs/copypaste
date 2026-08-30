export type ImeFact = boolean | "unknown";

export interface ImeWindowFrame {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface ImeRuntimeDiagnostics {
  inputShown: ImeFact;
  reportedImeVisible: ImeFact;
  imeWindowPresent: ImeFact;
  imeWindowVisible: ImeFact;
  imeWindowFrame: ImeWindowFrame | "unknown";
}

function exactBoolean(output: string, name: string): ImeFact {
  const matches = Array.from(
    output.matchAll(new RegExp(`^\\s*${name}=(true|false)\\s*$`, "gm")),
  );
  if (matches.length !== 1) return "unknown";
  return matches[0]![1] === "true";
}

function reportedImeVisible(output: string): ImeFact {
  const matches = Array.from(output.matchAll(/^\s*mImeWindowVis=(\d+)\s*$/gm));
  if (matches.length !== 1) return "unknown";
  const flags = Number(matches[0]![1]);
  if (!Number.isSafeInteger(flags) || flags < 0 || flags > 3) return "unknown";
  return (flags & 0x2) !== 0;
}

function escapePattern(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function frame(section: string): ImeWindowFrame | "unknown" {
  const matches = Array.from(
    section.matchAll(
      /(?:mFrame=|\bframe=)\[(-?\d+),\s*(-?\d+)\]\[(-?\d+),\s*(-?\d+)\]/g,
    ),
  );
  if (matches.length !== 1) return "unknown";
  const [left, top, right, bottom] = matches[0]!.slice(1).map(Number);
  if (
    ![left, top, right, bottom].every(Number.isSafeInteger) ||
    right! < left! ||
    bottom! < top!
  ) {
    return "unknown";
  }
  return { left: left!, top: top!, width: right! - left!, height: bottom! - top! };
}

function imeWindowDiagnostics(windowDump: string): Pick<
  ImeRuntimeDiagnostics,
  "imeWindowPresent" | "imeWindowVisible" | "imeWindowFrame"
> {
  const markers = windowDump
    .split("\n")
    .filter((line) => /^\s*mInputMethodWindow=/.test(line));
  if (markers.length === 0) {
    return {
      imeWindowPresent: "unknown",
      imeWindowVisible: "unknown",
      imeWindowFrame: "unknown",
    };
  }
  if (markers.length !== 1) {
    return {
      imeWindowPresent: "unknown",
      imeWindowVisible: "unknown",
      imeWindowFrame: "unknown",
    };
  }
  const token = markers[0]!.match(/^\s*mInputMethodWindow=Window\{([^\s}]+)/)?.[1];
  if (!token) {
    return {
      imeWindowPresent: "unknown",
      imeWindowVisible: "unknown",
      imeWindowFrame: "unknown",
    };
  }
  const headers = Array.from(
    windowDump.matchAll(
      new RegExp(`^\\s*Window #\\d+ Window\\{${escapePattern(token)}(?:\\s|\\})`, "gm"),
    ),
  );
  if (headers.length !== 1 || headers[0]!.index === undefined) {
    return {
      imeWindowPresent: true,
      imeWindowVisible: "unknown",
      imeWindowFrame: "unknown",
    };
  }
  const start = headers[0]!.index;
  const bodyStart = windowDump.indexOf("\n", start);
  const next = bodyStart < 0
    ? -1
    : windowDump.slice(bodyStart + 1).search(/^\s*Window #\d+ Window\{/m);
  const section = next < 0
    ? windowDump.slice(start)
    : windowDump.slice(start, bodyStart + 1 + next);
  return {
    imeWindowPresent: true,
    imeWindowVisible: exactBoolean(section, "isVisible"),
    imeWindowFrame: frame(section),
  };
}

export function parseImeRuntimeDiagnostics(
  inputMethodDump: string | undefined,
  windowDump: string | undefined,
): ImeRuntimeDiagnostics {
  const inputMethod = inputMethodDump ?? "";
  if (windowDump === undefined) {
    return {
      inputShown: exactBoolean(inputMethod, "mInputShown"),
      reportedImeVisible: reportedImeVisible(inputMethod),
      imeWindowPresent: "unknown",
      imeWindowVisible: "unknown",
      imeWindowFrame: "unknown",
    };
  }
  return {
    inputShown: exactBoolean(inputMethod, "mInputShown"),
    reportedImeVisible: reportedImeVisible(inputMethod),
    ...imeWindowDiagnostics(windowDump),
  };
}
