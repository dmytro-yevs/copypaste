import type { LucideIcon } from "lucide-react";
import {
  AppWindow,
  Braces,
  Code2,
  Compass,
  File,
  FileImage,
  FileText,
  Folder,
  Hash,
  Globe2,
  Image,
  Link2,
  Mail,
  MessageCircle,
  Palette,
  ShieldAlert,
  Send,
  TextCursorInput,
  CircleHelp,
} from "lucide-react";

import type { Kind } from "@/lib/format";
import type { Item } from "@/lib/ipc";

export interface ClipTypeMetadata {
  readonly label: string;
  readonly Icon: LucideIcon;
}

const TYPE: Record<Kind, ClipTypeMetadata> = {
  secret: { label: "Sensitive", Icon: ShieldAlert },
  image: { label: "Image", Icon: FileImage },
  file: { label: "File", Icon: File },
  url: { label: "Link", Icon: Link2 },
  mail: { label: "Email", Icon: Mail },
  path: { label: "File path", Icon: Folder },
  json: { label: "JSON", Icon: Braces },
  code: { label: "Code", Icon: Code2 },
  color: { label: "Color", Icon: Palette },
  num: { label: "Number", Icon: Hash },
  text: { label: "Text", Icon: FileText },
  unknown: { label: "Other", Icon: TextCursorInput },
};

/** Content-type presentation lives in one place so history and its mobile
 * layout never gradually learn different meanings for the same clip. */
export function clipTypeMetadata(kind: Kind): ClipTypeMetadata {
  return TYPE[kind];
}

const KNOWN_SOURCE_APPS: Readonly<Record<string, Omit<ClipSourceMetadata, "available">>> = {
  "com.apple.Safari": { label: "Safari", Icon: Compass },
  "com.apple.finder": { label: "Finder", Icon: Folder },
  "com.apple.mail": { label: "Mail", Icon: Mail },
  "com.apple.Notes": { label: "Notes", Icon: FileText },
  "com.apple.MobileSMS": { label: "Messages", Icon: MessageCircle },
  "com.apple.TextEdit": { label: "TextEdit", Icon: FileText },
  "com.apple.Terminal": { label: "Terminal", Icon: TextCursorInput },
  "com.google.Chrome": { label: "Google Chrome", Icon: Globe2 },
  "org.mozilla.firefox": { label: "Firefox", Icon: Globe2 },
  "com.google.android.gm": { label: "Gmail", Icon: Mail },
  "com.google.android.apps.photos": { label: "Google Photos", Icon: Image },
  "com.android.chrome": { label: "Chrome", Icon: Globe2 },
  "com.android.documentsui": { label: "Files", Icon: Folder },
  "com.microsoft.VSCode": { label: "Visual Studio Code", Icon: Code2 },
  "com.tinyspeck.slackmacgap": { label: "Slack", Icon: Hash },
  "org.telegram.desktop": { label: "Telegram", Icon: Send },
};

export interface ClipSourceMetadata {
  readonly label: string;
  readonly Icon: LucideIcon;
  readonly available: boolean;
}

function appName(bundleId: string): string | null {
  if (!/^[a-z0-9]+(?:[._-][a-z0-9]+)+$/i.test(bundleId)) return null;
  const candidate = bundleId.split(".").at(-1)?.replaceAll(/[-_]+/g, " ").trim();
  if (!candidate || !/^[a-z0-9 ]+$/i.test(candidate)) return null;
  return candidate.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function suppliedAppName(name: string | null | undefined): string | null {
  const trimmed = name?.trim();
  return trimmed && trimmed.length <= 120 ? trimmed : null;
}

export function clipSourceMetadata(item: Item): ClipSourceMetadata {
  const bundleId = item.source_app_bundle_id;
  const label = suppliedAppName(item.source_app_name);
  if (!bundleId) {
    return label
      ? { label, Icon: AppWindow, available: true }
      : { label: "Unknown app", Icon: CircleHelp, available: false };
  }
  const known = KNOWN_SOURCE_APPS[bundleId];
  if (known) return { ...known, label: label ?? known.label, available: true };
  const name = appName(bundleId);
  return name
    ? { label: label ?? name, Icon: AppWindow, available: true }
    : { label: "Unknown app", Icon: CircleHelp, available: false };
}
