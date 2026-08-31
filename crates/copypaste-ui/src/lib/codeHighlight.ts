import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import shell from "highlight.js/lib/languages/shell";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import { createLowlight } from "lowlight";

import type { Kind } from "@/lib/format";

export const CODE_LANGUAGES = [
  "json", "javascript", "typescript", "python", "bash", "shell",
  "rust", "go", "java", "c", "cpp", "csharp", "css", "xml",
  "sql", "yaml", "markdown", "diff",
] as const;

const highlighter = createLowlight();
highlighter.register("json", json);
highlighter.register("javascript", javascript);
highlighter.register("typescript", typescript);
highlighter.register("python", python);
highlighter.register("bash", bash);
highlighter.register("shell", shell);
highlighter.register("rust", rust);
highlighter.register("go", go);
highlighter.register("java", java);
highlighter.register("c", c);
highlighter.register("cpp", cpp);
highlighter.register("csharp", csharp);
highlighter.register("css", css);
highlighter.register("xml", xml);
highlighter.register("sql", sql);
highlighter.register("yaml", yaml);
highlighter.register("markdown", markdown);
highlighter.register("diff", diff);

const CARD_LIMIT = 8 * 1024;
const READER_LIMIT = 64 * 1024;
const MIN_RELEVANCE = 2;
type HighlightTree = ReturnType<typeof highlighter.highlight>;

export interface CodeHighlightResult {
  readonly tree: HighlightTree | null;
  readonly language: string | null;
  readonly label: string;
  readonly highlightedText: string;
  readonly remainder: string;
}

function safeNode(node: unknown): boolean {
  if (typeof node !== "object" || node === null || !("type" in node)) return false;
  if (node.type === "text") return "value" in node && typeof node.value === "string";
  if (
    node.type !== "element" ||
    !("tagName" in node) ||
    node.tagName !== "span" ||
    !("properties" in node) ||
    typeof node.properties !== "object" ||
    node.properties === null ||
    !("children" in node) ||
    !Array.isArray(node.children)
  ) return false;
  const properties = node.properties as Record<string, unknown>;
  const classes = properties.className;
  return Object.keys(properties).every((key) => key === "className") &&
    (classes === undefined ||
      (Array.isArray(classes) &&
        classes.every((value) => typeof value === "string" && /^hljs-[a-z0-9_-]+$/.test(value)))) &&
    node.children.every(safeNode);
}

function languageLabel(language: string | null): string {
  if (language === null) return "Unknown";
  const labels: Readonly<Record<string, string>> = {
    bash: "Bash", c: "C", cpp: "C++", csharp: "C#", css: "CSS",
    diff: "Diff", go: "Go", java: "Java", javascript: "JavaScript",
    json: "JSON", markdown: "Markdown", python: "Python", rust: "Rust",
    shell: "Shell", sql: "SQL", typescript: "TypeScript",
    xml: "HTML/XML", yaml: "YAML",
  };
  return labels[language] ?? "Unknown";
}

export function detectCode(
  value: string,
  kind: Kind,
  mode: "card" | "inspector" | "expanded",
): CodeHighlightResult {
  const limit = mode === "card" ? CARD_LIMIT : READER_LIMIT;
  const highlightedText = value.slice(0, limit);
  const remainder = value.slice(limit);
  const fallback = { tree: null, language: null, label: "Unknown", highlightedText, remainder } as const;
  if (kind !== "code" && kind !== "json") return fallback;
  try {
    const tree = kind === "json"
      ? highlighter.highlight("json", highlightedText)
      : highlighter.highlightAuto(highlightedText, { subset: CODE_LANGUAGES });
    const language = typeof tree.data?.language === "string" ? tree.data.language : null;
    const relevance = typeof tree.data?.relevance === "number" ? tree.data.relevance : 0;
    const accepted = kind === "json" || (language !== null && relevance >= MIN_RELEVANCE);
    if (!accepted || !tree.children.every(safeNode)) return fallback;
    return { tree, language, label: languageLabel(language), highlightedText, remainder };
  } catch {
    return fallback;
  }
}
