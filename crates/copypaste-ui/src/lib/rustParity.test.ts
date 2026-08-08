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
import {
  MAX_DECODED_IMAGE_MB as DECODED_IMAGE_CHOICES,
  MAX_FILE_SIZE_BYTES as FILE_CHOICES,
  MAX_FILE_SIZE_BYTES_LIMIT,
  MAX_IMAGE_SIZE_BYTES as IMAGE_CHOICES,
  MAX_TEXT_SIZE_BYTES as TEXT_CHOICES,
  MIN_DECODED_IMAGE_MB,
  MIN_FILE_SIZE_BYTES,
  MIN_IMAGE_SIZE_BYTES,
  MIN_TEXT_SIZE_BYTES,
  POLL_INTERVAL_MAX_MS,
  POLL_INTERVAL_MIN_MS,
} from "@/components/settings/serviceChoices";
import { EVENT_CAPTURE_STATE, EVENT_CAPTURED } from "@/hooks/useCapture";
import {
  EVENT_AUTOSTART_CHANGED,
  EVENT_CHANGED,
  EVENT_PUSH_STATE,
} from "@/hooks/usePush";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";
import { PAGE_SIZE, SEARCH_LIMIT } from "@/lib/layout";
import { DEFAULT_SHORTCUT } from "@/lib/accelerator";

// jsdom serves `import.meta.url` over http, so the crates directory is reached
// from the vitest root instead.
const CRATES = path.resolve(process.cwd(), "..");

const IPC_LIB = "copypaste-ipc/src/lib.rs";
const IPC_CONFIG = "copypaste-ipc/src/config.rs";
const IPC_LIMITS = "copypaste-ipc/src/limits.rs";
const P2P_PEERS = "copypaste-p2p/src/peers/mod.rs";
const PUSH = "copypaste-ui/src-tauri/src/service/push.rs";
const INTAKE = "copypaste-ui/src-tauri/src/capture/intake.rs";
const SHORTCUT = "copypaste-ui/src-tauri/src/shell/shortcut.rs";
const AUTOSTART = "copypaste-ui/src-tauri/src/shell/autostart.rs";

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

function product(file: string, name: string): number {
  return value(file, name)
    .split("*")
    .map((factor) => Number(factor.trim().replace(/_/g, "")))
    .reduce((result, factor) => result * factor, 1);
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

  test(`${AUTOSTART} emits what usePush subscribes to`, () => {
    expect(rustEvents(AUTOSTART)).toEqual({
      EVENT_CHANGED: EVENT_AUTOSTART_CHANGED,
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

test(`service capture bounds match ${IPC_CONFIG}`, () => {
  expect(POLL_INTERVAL_MIN_MS).toBe(product(IPC_CONFIG, "POLL_INTERVAL_MIN_MS"));
  expect(POLL_INTERVAL_MAX_MS).toBe(product(IPC_CONFIG, "POLL_INTERVAL_MAX_MS"));
  expect(MIN_TEXT_SIZE_BYTES).toBe(product(IPC_CONFIG, "MIN_TEXT_SIZE_BYTES"));
  expect(MIN_IMAGE_SIZE_BYTES).toBe(product(IPC_CONFIG, "MIN_IMAGE_SIZE_BYTES"));
  expect(MIN_FILE_SIZE_BYTES).toBe(product(IPC_CONFIG, "MIN_FILE_SIZE_BYTES"));
  expect(MAX_FILE_SIZE_BYTES_LIMIT).toBe(product(IPC_CONFIG, "MAX_FILE_SIZE_BYTES"));
  expect(MIN_DECODED_IMAGE_MB).toBe(product(IPC_CONFIG, "MIN_DECODED_IMAGE_MB"));
});

test("service choices include every capture default and binding boundary", () => {
  const values = (choices: readonly { readonly value: number }[]) =>
    choices.map((choice) => choice.value);
  expect(values(TEXT_CHOICES)).toEqual(
    expect.arrayContaining([
      product(IPC_CONFIG, "MIN_TEXT_SIZE_BYTES"),
      product(IPC_CONFIG, "MAX_TEXT_SIZE_BYTES"),
    ]),
  );
  expect(values(IMAGE_CHOICES)).toEqual(
    expect.arrayContaining([
      product(IPC_CONFIG, "MIN_IMAGE_SIZE_BYTES"),
      product(IPC_CONFIG, "MAX_IMAGE_SIZE_BYTES"),
    ]),
  );
  expect(values(FILE_CHOICES)).toEqual(
    expect.arrayContaining([
      product(IPC_CONFIG, "MIN_FILE_SIZE_BYTES"),
      product(IPC_CONFIG, "MAX_FILE_SIZE_BYTES"),
    ]),
  );
  expect(Math.max(...values(FILE_CHOICES))).toBe(MAX_FILE_SIZE_BYTES_LIMIT);
  expect(values(DECODED_IMAGE_CHOICES)).toEqual(
    expect.arrayContaining([
      product(IPC_CONFIG, "MIN_DECODED_IMAGE_MB"),
      product(IPC_CONFIG, "MAX_DECODED_IMAGE_MB"),
    ]),
  );
});
