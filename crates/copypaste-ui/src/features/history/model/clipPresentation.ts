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

type RowLabelKey =
  | "history.row.sensitiveName"
  | "history.row.potentialSensitiveWarning"
  | "history.row.empty"
  | "history.row.pinnedPrefix"
  | "history.row.fromPrefix"
  | "history.row.wontSync";

const TYPE: Record<Kind, ClipTypeMetadata> = {
  secret: { label: "Sensitive", icon: "sensitive" },
  image: { label: "Image", icon: "fileImage" },
  file: { label: "File", icon: "file" },
  url: { label: "Link", icon: "link" },
  mail: { label: "Email", icon: "mail" },
  path: { label: "File path", icon: "folder" },
  json: { label: "JSON", icon: "braces" },
  code: { label: "Code", icon: "code" },
  color: { label: "Color", icon: "palette" },
  num: { label: "Number", icon: "hash" },
  text: { label: "Text", icon: "fileText" },
  unknown: { label: "Other", icon: "text" },
};

export function clipTypeMetadata(kind: Kind, content = ""): ClipTypeMetadata {
  if (
    (kind === "file" || kind === "path") &&
    /\.(?:html?|xhtml|xml|svg|css|[cm]?[jt]sx?|json|ya?ml|rs|go|py|java|c|cc|cpp|h|hpp|cs|sql|sh|bash)$/i.test(content.trim())
  ) {
    return { label: "Source file", icon: "code" };
  }
  return TYPE[kind];
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
          ? "Image"
          : (preview ?? previewOf(item.content));
  const named = item.pinned ? `${translate("history.row.pinnedPrefix")} ${body}` : body;
  const marks: string[] = [];
  if (origin !== null) marks.push(`${translate("history.row.fromPrefix")} ${origin}`);
  if (wontSync(item)) marks.push(translate("history.row.wontSync"));
  return marks.length === 0 ? named : `${named} · ${marks.join(" · ")}`;
}
