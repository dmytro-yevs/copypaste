import type { Item } from "@/lib/ipc";

export type ClipPresentationIcon =
  | "app"
  | "braces"
  | "code"
  | "compass"
  | "file"
  | "fileImage"
  | "fileText"
  | "folder"
  | "globe"
  | "hash"
  | "image"
  | "link"
  | "mail"
  | "messages"
  | "palette"
  | "sensitive"
  | "terminal"
  | "text"
  | "unknown";

export interface ClipSourceMetadata {
  readonly label: string;
  readonly icon: ClipPresentationIcon;
  readonly available: boolean;
}

const KNOWN_SOURCE_APPS: Readonly<Record<string, Omit<ClipSourceMetadata, "available">>> = {
  "com.apple.Safari": { label: "Safari", icon: "compass" },
  "com.apple.finder": { label: "Finder", icon: "folder" },
  "com.apple.mail": { label: "Mail", icon: "mail" },
  "com.apple.Notes": { label: "Notes", icon: "fileText" },
  "com.apple.MobileSMS": { label: "Messages", icon: "messages" },
  "com.apple.TextEdit": { label: "TextEdit", icon: "fileText" },
  "com.apple.Terminal": { label: "Terminal", icon: "terminal" },
  "com.google.Chrome": { label: "Google Chrome", icon: "globe" },
  "org.mozilla.firefox": { label: "Firefox", icon: "globe" },
  "com.google.android.gm": { label: "Gmail", icon: "mail" },
  "com.google.android.apps.photos": { label: "Google Photos", icon: "image" },
  "com.android.chrome": { label: "Chrome", icon: "globe" },
  "com.android.documentsui": { label: "Files", icon: "folder" },
  "com.microsoft.VSCode": { label: "Visual Studio Code", icon: "code" },
  "com.tinyspeck.slackmacgap": { label: "Slack", icon: "hash" },
  "org.telegram.desktop": { label: "Telegram", icon: "mail" },
};

function appName(bundleId: string): string | null {
  if (!/^[a-z0-9]+(?:[._-][a-z0-9]+)+$/i.test(bundleId)) return null;
  const parts = bundleId.split(".");
  const candidate = parts[parts.length - 1]?.replace(/[-_]+/g, " ").trim();
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
      ? { label, icon: "app", available: true }
      : { label: "Unknown app", icon: "unknown", available: false };
  }
  const known = KNOWN_SOURCE_APPS[bundleId];
  if (known) return { ...known, label: label ?? known.label, available: true };
  const name = appName(bundleId);
  return name
    ? { label: label ?? name, icon: "app", available: true }
    : { label: "Unknown app", icon: "unknown", available: false };
}
