import pathBrowserify from "path-browserify-win32";

import { t } from "@/i18n";
import type { Kind } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import { previewOf } from "@/lib/format";
import type { ClipPresentationIcon } from "@/lib/clipSourcePresentation";
import { wontSync } from "./origin";

export { clipSourceMetadata } from "@/lib/clipSourcePresentation";
export type {
  ClipPresentationIcon,
  ClipSourceMetadata,
} from "@/lib/clipSourcePresentation";

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

export interface ClipCopyActionPresentation {
  readonly icon: "copy" | "image";
  readonly label: string;
}

type RowLabelKey =
  | "history.row.sensitiveName"
  | "history.row.potentialSensitiveWarning"
  | "history.row.empty"
  | "history.row.pinnedPrefix"
  | "history.row.fromPrefix"
  | "history.row.wontSync";

const KIND = {
  secret: {
    singular: "history.type.secret",
    filter: "history.kind.secret",
    icon: "sensitive",
  },
  image: {
    singular: "history.type.image",
    filter: "history.kind.image",
    icon: "fileImage",
  },
  file: {
    singular: "history.type.file",
    filter: "history.kind.file",
    icon: "file",
  },
  url: {
    singular: "history.type.url",
    filter: "history.kind.url",
    icon: "link",
  },
  mail: {
    singular: "history.type.mail",
    filter: "history.kind.mail",
    icon: "mail",
  },
  path: {
    singular: "history.type.path",
    filter: "history.kind.path",
    icon: "folder",
  },
  json: {
    singular: "history.type.json",
    filter: "history.kind.json",
    icon: "braces",
  },
  code: {
    singular: "history.type.code",
    filter: "history.kind.code",
    icon: "code",
  },
  color: {
    singular: "history.type.color",
    filter: "history.kind.color",
    icon: "palette",
  },
  num: {
    singular: "history.type.num",
    filter: "history.kind.num",
    icon: "hash",
  },
  text: {
    singular: "history.type.text",
    filter: "history.kind.text",
    icon: "fileText",
  },
  unknown: {
    singular: "history.type.unknown",
    filter: "history.kind.unknown",
    icon: "text",
  },
} as const satisfies Record<
  Kind,
  {
    readonly singular: `history.type.${Kind}`;
    readonly filter: `history.kind.${Kind}`;
    readonly icon: ClipPresentationIcon;
  }
>;

const win32 = (pathBrowserify as {
  win32: typeof import("node:path").win32;
}).win32;

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

export function clipCopyAction(kind: Kind): ClipCopyActionPresentation {
  return kind === "image"
    ? { icon: "image", label: t("history.detail.copyImage") }
    : { icon: "copy", label: t("history.detail.copy") };
}

export function historyKindFilterLabel(kind: "all" | Kind): string {
  return kind === "all" ? t("history.kind.all") : t(KIND[kind].filter);
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
  const presentation = KIND[kind];
  return { label: t(presentation.singular), icon: presentation.icon };
}

export function rowLabel(
  item: Item,
  origin: string | null,
  preview: string | undefined,
  translate: (key: RowLabelKey) => string,
): string {
  const body = item.is_sensitive
    ? translate("history.row.sensitiveName")
    : item.sensitive_finding !== null
      ? `${translate("history.row.potentialSensitiveWarning")}. ${item.sensitive_finding.redacted_preview}`
      : item.content === null
        ? translate("history.row.empty")
        : item.content_type.toLowerCase().startsWith("image/")
          ? clipTypeMetadata("image").label
          : (preview ?? previewOf(item.content));
  const named = item.pinned ? `${translate("history.row.pinnedPrefix")} ${body}` : body;
  const marks: string[] = [];
  if (origin !== null) marks.push(`${translate("history.row.fromPrefix")} ${origin}`);
  if (wontSync(item)) marks.push(translate("history.row.wontSync"));
  return marks.length === 0 ? named : `${named} · ${marks.join(" · ")}`;
}
