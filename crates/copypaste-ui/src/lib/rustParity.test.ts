/**
 * Constants this frontend restates from Rust, checked against the Rust source.
 *
 * Nothing else connects them: the values below are not on the wire, so a change
 * on one side compiles clean on both and fails at runtime — a false
 * version-mismatch banner, a device cap that refuses a pairing the UI said was
 * allowed, or a `listen()` on a name nothing emits, which kills live updates
 * silently and looks like a hung backend.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, test } from "vitest";

import { MAX_PAIRINGS } from "@/components/devices/peerState";
import { EVENT_CAPTURE_STATE, EVENT_CAPTURED } from "@/hooks/useCapture";
import { EVENT_CHANGED, EVENT_PUSH_STATE } from "@/hooks/usePush";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";
import { PAGE_SIZE, SEARCH_LIMIT } from "@/lib/layout";
import { DEFAULT_SHORTCUT } from "@/lib/accelerator";

// jsdom serves `import.meta.url` over http, so the crates directory is reached
// from the vitest root instead.
const CRATES = path.resolve(process.cwd(), "..");

const IPC_LIB = "copypaste-ipc/src/lib.rs";
const IPC_LIMITS = "copypaste-ipc/src/limits.rs";
const P2P_PEERS = "copypaste-p2p/src/peers/mod.rs";
const PUSH = "copypaste-ui/src-tauri/src/service/push.rs";
const INTAKE = "copypaste-ui/src-tauri/src/capture/intake.rs";
const SHORTCUT = "copypaste-ui/src-tauri/src/shell/shortcut.rs";

function rust(file: string): string {
  return readFileSync(path.join(CRATES, file), "utf8");
}

function value(file: string, name: string): string {
  const found = new RegExp(`\\bconst ${name}\\s*:[^=]+=\\s*([^;]+);`).exec(
    rust(file),
  );
  if (!found) {
    throw new Error(`${name} is no longer defined in ${file}`);
  }
  return found[1].trim();
}

function number(file: string, name: string): number {
  return Number(value(file, name).replace(/_/g, ""));
}

/** Every `EVENT_*` the Tauri side can emit, by constant name. */
function rustEvents(file: string): Record<string, string> {
  const events: Record<string, string> = {};
  for (const [, name, literal] of rust(file).matchAll(
    /\bconst (EVENT_\w+)\s*:\s*&'?\w*\s*str\s*=\s*"([^"]*)"/g,
  )) {
    events[name] = literal;
  }
  return events;
}

test(`CURRENT_PROTOCOL_VERSION matches PROTOCOL_VERSION in ${IPC_LIB}`, () => {
  expect(CURRENT_PROTOCOL_VERSION).toBe(number(IPC_LIB, "PROTOCOL_VERSION"));
});

test(`MAX_PAIRINGS matches ${P2P_PEERS}`, () => {
  expect(MAX_PAIRINGS).toBe(number(P2P_PEERS, "MAX_PAIRINGS"));
});

test(`DEFAULT_SHORTCUT matches ${SHORTCUT}`, () => {
  expect(DEFAULT_SHORTCUT).toBe(value(SHORTCUT, "DEFAULT_SHORTCUT").replaceAll('"', ""));
});

describe("event names the frontend listens for", () => {
  test(`${PUSH} emits exactly what usePush subscribes to`, () => {
    expect(rustEvents(PUSH)).toEqual({
      EVENT_CHANGED,
      EVENT_PUSH_STATE,
    });
  });

  test(`${INTAKE} emits exactly what useCapture subscribes to`, () => {
    expect(rustEvents(INTAKE)).toEqual({
      EVENT_CAPTURED,
      EVENT_CAPTURE_STATE,
    });
  });
});

/**
 * Not a mirrored value — a bound. Both servers clamp silently, so a request
 * over the ceiling returns a short page that reads as "the history ends here".
 */
test(`the page sizes asked for fit MAX_PAGE in ${IPC_LIMITS}`, () => {
  const maxPage = number(IPC_LIMITS, "MAX_PAGE");
  expect(PAGE_SIZE).toBeLessThanOrEqual(maxPage);
  expect(SEARCH_LIMIT).toBeLessThanOrEqual(maxPage);
});
