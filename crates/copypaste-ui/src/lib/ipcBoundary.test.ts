import { readdirSync, readFileSync } from "node:fs";
import { extname, resolve } from "node:path";

import { parse } from "@babel/parser";
import { describe, expect, it } from "vitest";

interface AstNode {
  readonly type: string;
  readonly [key: string]: unknown;
}

const SOURCE_ROOT = resolve(process.cwd(), "src");
const IPC_CALL = resolve(SOURCE_ROOT, "lib/ipcCall.ts");

function productionModules(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) return productionModules(path);
    if (![".ts", ".tsx"].includes(extname(path))) return [];
    if (/\.(?:test|typecheck)\.tsx?$/.test(path)) return [];
    return [path];
  });
}

function nodes(value: unknown): AstNode[] {
  if (Array.isArray(value)) return value.flatMap(nodes);
  if (typeof value !== "object" || value === null) return [];
  const record = value as Record<string, unknown>;
  const nested = Object.entries(record)
    .filter(([key]) => !["loc", "start", "end"].includes(key))
    .flatMap(([, child]) => nodes(child));
  return typeof record.type === "string"
    ? [record as unknown as AstNode, ...nested]
    : nested;
}

function rawInvokeImports(path: string): string[] {
  const source = readFileSync(path, "utf8");
  const ast = parse(source, {
    sourceType: "module",
    plugins: ["typescript", ...(path.endsWith(".tsx") ? ["jsx" as const] : [])],
  });
  return nodes(ast).flatMap((node) => {
    if (node.type !== "ImportDeclaration") return [];
    const imported = node.source as { value?: unknown } | undefined;
    if (imported?.value !== "@tauri-apps/api/core") return [];
    const specifiers = Array.isArray(node.specifiers) ? node.specifiers : [];
    return specifiers.flatMap((specifier) => {
      const record = specifier as Record<string, unknown>;
      if (record.type === "ImportNamespaceSpecifier") return [path];
      const name = (record.imported as { name?: unknown } | undefined)?.name;
      return name === "invoke" ? [path] : [];
    });
  });
}

describe("IPC boundary", () => {
  it("keeps raw Tauri invoke inside ipcCall", () => {
    const violations = productionModules(SOURCE_ROOT)
      .filter((path) => path !== IPC_CALL)
      .flatMap(rawInvokeImports);

    expect(violations).toEqual([]);
  });
});
