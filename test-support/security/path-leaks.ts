import { readFileSync } from "node:fs";

export type PathLeakPattern = Readonly<{
  id: string;
  pattern: RegExp;
}>;

export type PathLeak = Readonly<{
  rule: string;
  match: string;
}>;

type DisplayLeakCase = Readonly<{
  id: string;
  platform?: "android";
  surface: string;
  expectedRule: string;
}>;

type SafeDisplayCase = Readonly<{
  id: string;
  surface: string;
}>;

type ArtifactContainmentCase = Readonly<{
  id: string;
  kind: string;
  root: string;
  entries: readonly string[];
  expected: "reject";
}>;

export type PathSecurityVectors = Readonly<{
  schemaVersion: 1;
  displayLeakCases: readonly DisplayLeakCase[];
  safeDisplayCases: readonly SafeDisplayCase[];
  artifactContainmentCases: readonly ArtifactContainmentCase[];
}>;

const BOUNDARY = String.raw`(?:^|[\s"'(<\[{:=>])`;
const SEGMENT = String.raw`[^\s/\\:;"'<>|?*]+`;

const SHARED_PATTERNS: readonly PathLeakPattern[] = [
  {
    id: "file-uri",
    pattern: /\bfile:\/\/(?:\/|localhost\/)[^\s"'<>]+/imu,
  },
  {
    id: "parent-traversal",
    pattern: new RegExp(String.raw`${BOUNDARY}\.\.[\\/]${SEGMENT}`, "imu"),
  },
  {
    id: "home-relative",
    pattern: new RegExp(String.raw`${BOUNDARY}~[\\/]${SEGMENT}`, "imu"),
  },
  {
    id: "windows-unc",
    pattern: new RegExp(String.raw`${BOUNDARY}\\\\${SEGMENT}\\${SEGMENT}`, "imu"),
  },
  {
    id: "windows-drive",
    pattern: new RegExp(String.raw`${BOUNDARY}[A-Za-z]:[\\/]${SEGMENT}`, "mu"),
  },
  {
    id: "absolute-posix",
    pattern: new RegExp(String.raw`${BOUNDARY}/(?!/)${SEGMENT}(?:/${SEGMENT})*`, "mu"),
  },
  {
    id: "path-variable",
    pattern: /(?:\$HOME|\$XDG_[A-Z_]+|%APPDATA%|%LOCALAPPDATA%|%USERPROFILE%)/imu,
  },
];

export const DESKTOP_PATH_LEAK_PATTERNS: readonly PathLeakPattern[] = [
  { id: "desktop-socket", pattern: /\b[^\s"'<>]+\.sock\b/imu },
];

export const ANDROID_PATH_LEAK_PATTERNS: readonly PathLeakPattern[] = [
  { id: "android-private-database", pattern: /\bcopypaste-v2\.db\b/imu },
  { id: "android-socket", pattern: /\b[^\s"'<>]+\.sock\b/imu },
];

function firstMatch(surface: string, rule: PathLeakPattern): PathLeak | undefined {
  const flags = rule.pattern.flags.replaceAll("g", "");
  const match = new RegExp(rule.pattern.source, flags).exec(surface)?.[0];
  return match === undefined ? undefined : { rule: rule.id, match: match.trimStart() };
}

export function findFilesystemPathLeaks(
  surface: string,
  options: Readonly<{
    forbidden?: readonly string[];
    additions?: readonly PathLeakPattern[];
  }> = {},
): PathLeak[] {
  const leaks: PathLeak[] = [];
  for (const literal of options.forbidden ?? []) {
    if (literal && surface.includes(literal)) leaks.push({ rule: "forbidden-literal", match: literal });
  }
  for (const rule of [...SHARED_PATTERNS, ...(options.additions ?? [])]) {
    const leak = firstMatch(surface, rule);
    if (leak !== undefined) leaks.push(leak);
  }
  return leaks;
}

const document = JSON.parse(
  readFileSync(new URL("./path-security-vectors.json", import.meta.url), "utf8"),
) as PathSecurityVectors;

if (
  document.schemaVersion !== 1 ||
  !Array.isArray(document.displayLeakCases) ||
  !Array.isArray(document.safeDisplayCases) ||
  !Array.isArray(document.artifactContainmentCases)
) {
  throw new Error("unsupported path-security vector schema");
}

export const PATH_SECURITY_VECTORS = document;
