/**
 * Constants this frontend restates from Rust, checked against the Rust source.
 *
 * Nothing else connects them: the values below are not on the wire, so a change
 * on one side compiles clean on both and fails at runtime — a false
 * version-mismatch banner, a device cap that refuses a pairing the UI said was
 * allowed, or a `listen()` on a name nothing emits, which kills live updates
 * silently and looks like a hung backend.
 */
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import { parse } from "@babel/parser";
import traverse from "@babel/traverse";
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
import {
  EVENT_AUTOSTART_CHANGED,
  EVENT_CAPTURE_STATE,
  EVENT_CAPTURED,
  EVENT_CHANGED,
  EVENT_OPEN_SETTINGS,
  EVENT_PRIVATE_MODE_CHANGED,
  EVENT_PUSH_STATE,
  TAURI_EVENT_NAMES,
} from "@/lib/tauriEvents";
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
const SHORTCUT = "copypaste-ui/src-tauri/src/shell/shortcut.rs";
const UI_SRC = path.resolve(process.cwd(), "src");
const TAURI_SRC = path.resolve(process.cwd(), "src-tauri/src");
const GENERATED_IPC = path.join(UI_SRC, "generated/ipc.ts");

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

function sourceFiles(root: string, extensions: ReadonlySet<string>): string[] {
  const found: string[] = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const full = path.join(root, entry.name);
    if (entry.isDirectory()) {
      found.push(...sourceFiles(full, extensions));
    } else if (extensions.has(path.extname(entry.name))) {
      found.push(full);
    }
  }
  return found;
}

function frontendListenersInSource(file: string, text: string): string[] {
  const listeners: string[] = [];
  const ast = parse(text, {
    sourceType: "module",
    plugins: file.endsWith(".tsx") ? ["typescript", "jsx"] : ["typescript"],
  });
  traverse(ast, {
    CallExpression(call) {
      if (call.node.callee.type !== "Identifier") return;
      const binding = call.scope.getBinding(call.node.callee.name);
      if (!binding?.path.isImportSpecifier()) return;
      const imported = binding.path.node.imported;
      const importedName =
        imported.type === "Identifier" ? imported.name : imported.value;
      if (
        importedName !== "listen" ||
        !binding.path.parentPath?.isImportDeclaration() ||
        binding.path.parentPath.node.source.value !== "@tauri-apps/api/event"
      ) {
        return;
      }
      const event = call.node.arguments[0];
      if (
        !event ||
        event.type === "ArgumentPlaceholder" ||
        event.type === "SpreadElement"
      ) {
        throw new Error(`listen() has no static event in ${file}`);
      }
      listeners.push(
        `${file.replace(/\\/g, "/")}:${text.slice(event.start ?? 0, event.end ?? 0)}`,
      );
    },
  });
  return listeners.sort();
}

function frontendListeners(): string[] {
  const listeners: string[] = [];
  for (const file of sourceFiles(UI_SRC, new Set([".ts", ".tsx"]))) {
    if (
      file.includes(`${path.sep}generated${path.sep}`) ||
      file.includes(".test.")
    ) {
      continue;
    }
    const text = readFileSync(file, "utf8");
    listeners.push(
      ...frontendListenersInSource(path.relative(UI_SRC, file), text),
    );
  }
  return listeners.sort();
}

function generatedEventNames(): string[] {
  const ast = parse(readFileSync(GENERATED_IPC, "utf8"), {
    sourceType: "module",
    plugins: ["typescript"],
  });
  const names: string[] = [];
  traverse(ast, {
    TSTypeAliasDeclaration(alias) {
      if (alias.node.id.name !== "TauriEventName") return;
      const members =
        alias.node.typeAnnotation.type === "TSUnionType"
          ? alias.node.typeAnnotation.types
          : [alias.node.typeAnnotation];
      for (const member of members) {
        if (
          member.type !== "TSLiteralType" ||
          member.literal.type !== "StringLiteral"
        ) {
          throw new Error("generated TauriEventName contains a non-string member");
        }
        names.push(member.literal.value);
      }
    },
  });
  if (names.length === 0) throw new Error("generated TauriEventName is missing");
  return names;
}

function rustEmitterContractsInSource(source: string): string[] {
  const contracts: string[] = [];
  const emitter =
    /^\s*(?:let\s+_\s*=\s*)?[A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*\.(emit|emit_to|emit_filter)\(\s*([^,\r\n]+),\s*(?:\r?\n\s*)?([^,\r\n]+)/gm;
  for (const match of source.matchAll(emitter)) {
    contracts.push((match[1] === "emit_to" ? match[3] : match[2]).trim());
  }
  return contracts;
}

function rustEmitterContracts(): string[] {
  const contracts: string[] = [];
  for (const file of sourceFiles(TAURI_SRC, new Set([".rs"]))) {
    contracts.push(...rustEmitterContractsInSource(readFileSync(file, "utf8")));
  }
  return contracts;
}

function unguardedEmitterContracts(contracts: readonly string[]): string[] {
  return contracts.filter(
    (contract) => !/^TauriEventName::\w+\.as_str\(\)$/.test(contract),
  );
}

function coverageErrors(
  actual: readonly string[],
  expected: readonly string[],
): string[] {
  const counts = (values: readonly string[]) => {
    const result = new Map<string, number>();
    for (const value of values) result.set(value, (result.get(value) ?? 0) + 1);
    return result;
  };
  const actualCounts = counts(actual);
  const expectedCounts = counts(expected);
  return [...new Set([...actualCounts.keys(), ...expectedCounts.keys()])]
    .filter((name) => actualCounts.get(name) !== expectedCounts.get(name))
    .sort();
}

const EVENT_CONTRACTS = [
  {
    listener: "App.tsx:EVENT_OPEN_SETTINGS",
    rust: "OpenSettings",
    event: EVENT_OPEN_SETTINGS,
  },
  {
    listener: "hooks/useCapture.ts:EVENT_CAPTURE_STATE",
    rust: "CaptureState",
    event: EVENT_CAPTURE_STATE,
  },
  {
    listener: "hooks/useCapture.ts:EVENT_CAPTURED",
    rust: "Captured",
    event: EVENT_CAPTURED,
  },
  {
    listener: "hooks/useDiagnostics.ts:EVENT_CHANGED",
    rust: "Changed",
    event: EVENT_CHANGED,
  },
  {
    listener: "hooks/usePush.ts:EVENT_CHANGED",
    rust: "Changed",
    event: EVENT_CHANGED,
  },
  {
    listener: "hooks/usePush.ts:EVENT_PUSH_STATE",
    rust: "PushState",
    event: EVENT_PUSH_STATE,
  },
  {
    listener: "hooks/usePush.ts:EVENT_PRIVATE_MODE_CHANGED",
    rust: "PrivateModeChanged",
    event: EVENT_PRIVATE_MODE_CHANGED,
  },
  {
    listener: "hooks/usePush.ts:EVENT_AUTOSTART_CHANGED",
    rust: "AutostartChanged",
    event: EVENT_AUTOSTART_CHANGED,
  },
] as const;

test(`CURRENT_PROTOCOL_VERSION matches PROTOCOL_VERSION in ${IPC_LIB}`, () => {
  expect(CURRENT_PROTOCOL_VERSION).toBe(number(IPC_LIB, "PROTOCOL_VERSION"));
});

test(`service payload ceilings match MAX_CONTENT_BYTES in ${IPC_LIMITS}`, () => {
  const hard = product(IPC_LIMITS, "MAX_CONTENT_BYTES");
  expect(MAX_FILE_SIZE_BYTES_LIMIT).toBe(hard);
});

test(`MAX_PAIRINGS matches ${P2P_PEERS}`, () => {
  expect(MAX_PAIRINGS).toBe(number(P2P_PEERS, "MAX_PAIRINGS"));
});

test(`DEFAULT_SHORTCUT matches ${SHORTCUT}`, () => {
  expect(DEFAULT_SHORTCUT).toBe(value(SHORTCUT, "DEFAULT_SHORTCUT").replace(/"/g, ""));
});

describe("event names the frontend listens for", () => {
  test("every listen call is covered exactly once", () => {
    const expected = EVENT_CONTRACTS.map(({ listener }) => listener).sort();
    expect(coverageErrors(frontendListeners(), expected)).toEqual([]);
  });

  test("the generated Rust union covers every frontend event", () => {
    expect([...TAURI_EVENT_NAMES].sort()).toEqual(generatedEventNames().sort());
    expect([...new Set(EVENT_CONTRACTS.map(({ event }) => event))].sort()).toEqual(
      [...TAURI_EVENT_NAMES].sort(),
    );
  });

  test("every listener contract has a Rust emitter", () => {
    const emitters = rustEmitterContracts();
    expect(emitters).not.toHaveLength(0);
    expect(unguardedEmitterContracts(emitters)).toEqual([]);
    const emitted = new Set(
      emitters.map((contract) =>
        contract.replace(/^TauriEventName::/, "").replace(/\.as_str\(\)$/, ""),
      ),
    );
    expect([...new Set(EVENT_CONTRACTS.map(({ rust }) => rust))].sort()).toEqual(
      [...emitted].sort(),
    );
  });

  test("listener discovery follows the imported binding", () => {
    const source = `
      import { listen as subscribe } from "@tauri-apps/api/event";
      subscribe(EVENT_ALIAS, () => {});
      function unrelated() {
        const listen = () => {};
        listen(EVENT_UNRELATED);
      }
    `;
    expect(frontendListenersInSource("synthetic.ts", source)).toEqual([
      "synthetic.ts:EVENT_ALIAS",
    ]);
  });

  test("emitter discovery guards emit, emit_to, and emit_filter", () => {
    const source = `
      let _ = app.emit(TauriEventName::Changed.as_str(), ());
      let _ = app.emit_to("main", "open-settings", ());
      let _ = app.emit_filter("private-mode-changed", (), |_| true);
    `;
    expect(
      unguardedEmitterContracts(rustEmitterContractsInSource(source)),
    ).toEqual(['"open-settings"', '"private-mode-changed"']);
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
  expect(Math.max(...values(TEXT_CHOICES))).toBe(MAX_FILE_SIZE_BYTES_LIMIT);
  expect(Math.max(...values(IMAGE_CHOICES))).toBe(MAX_FILE_SIZE_BYTES_LIMIT);
  expect(values(DECODED_IMAGE_CHOICES)).toEqual(
    expect.arrayContaining([
      product(IPC_CONFIG, "MIN_DECODED_IMAGE_MB"),
      product(IPC_CONFIG, "MAX_DECODED_IMAGE_MB"),
    ]),
  );
});

test(`service payload ceilings match MAX_CONTENT_BYTES in ${IPC_LIMITS}`, () => {
  const hard = product(IPC_LIMITS, "MAX_CONTENT_BYTES");
  expect(MAX_FILE_SIZE_BYTES_LIMIT).toBe(hard);
});
