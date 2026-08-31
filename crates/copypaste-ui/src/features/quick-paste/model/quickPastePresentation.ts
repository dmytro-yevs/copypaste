import { t } from "@/i18n";
import { kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";

export interface QuickPastePresentation {
  readonly rowLabel: string;
  readonly searchLabel: string;
}

export function quickPastePresentation(item: Item): QuickPastePresentation {
  if (item.is_sensitive) {
    return {
      rowLabel: t("quickPaste.row.sensitive"),
      searchLabel: "••••••••",
    };
  }
  const kind = kindOf(item);
  if (kind === "image") return { rowLabel: t("quickPaste.row.image"), searchLabel: t("quickPaste.row.image") };
  if (kind === "file") return { rowLabel: t("quickPaste.row.file"), searchLabel: t("quickPaste.row.file") };
  if (kind === "unknown") return { rowLabel: t("quickPaste.row.unsupported"), searchLabel: t("quickPaste.row.unsupported") };
  const label = item.sensitive_finding?.redacted_preview.trim() || item.content?.trim() || t("quickPaste.row.empty");
  return { rowLabel: label, searchLabel: label };
}
