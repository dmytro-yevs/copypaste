import pathBrowserify from "path-browserify-win32";

import { t } from "@/i18n";
import type { ClipPresentationIcon } from "@/lib/clipSourcePresentation";
import type { Kind } from "@/lib/format";
import type { Item } from "@/lib/ipc";

export interface ClipTypeMetadata {
  readonly label: string;
  readonly icon: ClipPresentationIcon;
}

export type ClipBodyPresentation =
  | { readonly state: "masked" }
  | { readonly state: "unavailable" }
  | {
      readonly state: "content";
      readonly content: string;
      readonly source: "preview" | "full" | "reveal" | "redacted";
    };

const TYPE = {
  secret: { label: "history.type.secret", icon: "sensitive" },
  image: { label: "history.type.image", icon: "fileImage" },
  file: { label: "history.type.file", icon: "file" },
  url: { label: "history.type.url", icon: "link" },
  mail: { label: "history.type.mail", icon: "mail" },
  path: { label: "history.type.path", icon: "folder" },
  json: { label: "history.type.json", icon: "braces" },
  code: { label: "history.type.code", icon: "code" },
  color: { label: "history.type.color", icon: "palette" },
  num: { label: "history.type.num", icon: "hash" },
  text: { label: "history.type.text", icon: "fileText" },
  unknown: { label: "history.type.unknown", icon: "text" },
} as const satisfies Record<Kind, { readonly label: `history.type.${Kind}`; readonly icon: ClipPresentationIcon }>;

const win32 = (pathBrowserify as { win32: typeof import("node:path").win32 }).win32;

export function resolveClipBodyPresentation({
  item,
  fullContent,
  fullContentFailed,
  revealedContent,
  showPotentialSensitiveOriginal = false,
}: {
  readonly item: Item;
  readonly fullContent: string | null;
  readonly fullContentFailed: boolean;
  readonly revealedContent: string | null;
  readonly showPotentialSensitiveOriginal?: boolean;
}): ClipBodyPresentation {
  if (item.is_sensitive) {
    return revealedContent === null
      ? { state: "masked" }
      : { state: "content", content: revealedContent, source: "reveal" };
  }
  if (item.truncated && fullContentFailed) return { state: "unavailable" };
  if (item.sensitive_finding !== null && !showPotentialSensitiveOriginal) {
    return {
      state: "content",
      content: item.sensitive_finding.redacted_preview,
      source: "redacted",
    };
  }
  if (fullContent !== null) {
    return { state: "content", content: fullContent, source: "full" };
  }
  return {
    state: "content",
    content: item.content ?? "",
    source: item.truncated ? "preview" : "full",
  };
}

export function fileDisplayName(content: string): string {
  const trimmed = content.trim();
  return win32.basename(trimmed) || content;
}

export function clipTypeMetadata(kind: Kind, content = ""): ClipTypeMetadata {
  if (
    (kind === "file" || kind === "path") &&
    /\.(?:html?|xhtml|xml|svg|css|[cm]?[jt]sx?|json|ya?ml|rs|go|py|java|c|cc|cpp|h|hpp|cs|sql|sh|bash)$/i.test(content.trim())
  ) {
    return { label: t("history.type.sourceFile"), icon: "code" };
  }
  const type = TYPE[kind];
  return { label: t(type.label), icon: type.icon };
}
