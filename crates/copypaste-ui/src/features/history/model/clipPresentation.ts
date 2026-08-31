import { t } from "@/i18n";
import { clipTypeMetadata } from "@/lib/clipPresentation";
import { kindOf, type Kind } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import { previewOf } from "@/lib/format";
import { wontSync } from "@/lib/itemOrigin";

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

export function clipCopyAction(kind: Kind): ClipCopyActionPresentation {
  return kind === "image"
    ? { icon: "image", label: t("history.detail.copyImage") }
    : { icon: "copy", label: t("history.detail.copy") };
}

export function historyKindFilterLabel(kind: "all" | Kind): string {
  return kind === "all" ? t("history.kind.all") : t(`history.kind.${kind}`);
}

export function rowLabel(
  item: Item,
  origin: string | null,
  preview: string | undefined,
  translate: (key: RowLabelKey) => string,
): string {
  let body: string;
  if (item.is_sensitive) {
    body = translate("history.row.sensitiveName");
  } else if (item.sensitive_finding !== null) {
    body = `${translate("history.row.potentialSensitiveWarning")}. ${item.sensitive_finding.redacted_preview}`;
  } else {
    const kind = kindOf(item);
    body = item.content_class !== "text"
      ? clipTypeMetadata(kind).label
      : item.content === null
        ? translate("history.row.empty")
        : (preview ?? previewOf(item.content));
  }
  const named = item.pinned ? `${translate("history.row.pinnedPrefix")} ${body}` : body;
  const marks: string[] = [];
  if (origin !== null) marks.push(`${translate("history.row.fromPrefix")} ${origin}`);
  if (wontSync(item)) marks.push(translate("history.row.wontSync"));
  return marks.length === 0 ? named : `${named} · ${marks.join(" · ")}`;
}
