import {
  Pin,
  PinOff,
  ShieldAlert,
} from "lucide-react";

import { type Item } from "@/lib/ipc";
import { t } from "@/i18n";
import { cn } from "@/lib/cn";
import { KIND_TEXT_CLASS, kindOf } from "@/lib/format";
import { imagePreviewHeight, imageRowHeight, rowHeight } from "@/lib/layout";
import { HistoryImagePreview } from "@/components/history/HistoryImagePreview";
import { SourceAppIcon } from "@/components/history/SourceAppIcon";
import { TimestampToggle } from "@/components/history/TimestampToggle";
import { isAndroidPlatform } from "@/lib/platform";
import {
  clipSourceMetadata,
  clipTypeMetadata,
} from "@/components/history/clipMetadata";

function isImage(item: Item) {
  return item.content_type.toLowerCase().startsWith("image/");
}

function label(item: Item) {
  if (item.is_sensitive) return t("quickPaste.row.sensitive");
  if (isImage(item)) return t("quickPaste.row.image");
  if (item.content_type.toLowerCase() === "file") return t("quickPaste.row.file");
  if (item.sensitive_finding) {
    return item.sensitive_finding.redacted_preview.trim() || t("quickPaste.row.empty");
  }
  return item.content?.trim() || t("quickPaste.row.empty");
}

interface QuickPasteRowProps {
  item: Item;
  active: boolean;
  /** Number of content lines visible in the compact surface. */
  previewLines: number;
  /** History owns the shared row density. Quick Paste only changes its
   * clipped text preview, never the card geometry. */
  rowPreviewLines: number;
  /** Quick Paste supports direct selection with ⌘1–9 on desktop. Keep that
   * hint beside the matching row action, rather than in a global legend. */
  shortcut: string | null;
  pinPending: boolean;
  onSelect: () => void;
  onCopy: () => void;
  onTogglePin: () => void;
}

export function QuickPasteRow({
  item,
  active,
  previewLines,
  rowPreviewLines,
  shortcut,
  pinPending,
  onSelect,
  onCopy,
  onTogglePin,
}: QuickPasteRowProps) {
  const image = isImage(item);
  const imageHeight = imagePreviewHeight(rowPreviewLines);
  const android = isAndroidPlatform();
  const text = label(item);
  const type = clipTypeMetadata(kindOf(item));
  const TypeIcon = type.Icon;
  const source = clipSourceMetadata(item);
  const SourceIcon = source.Icon;
  const potentiallySensitive = !item.is_sensitive && item.sensitive_finding !== null;

  return (
    <div
      role="listitem"
      aria-current={active || undefined}
      onMouseEnter={onSelect}
      className={cn(
        "group relative flex w-full items-start gap-[var(--gap-row)] overflow-hidden rounded-lg px-[var(--pad-row-x)] py-[var(--pad-row-y)] transition-colors duration-[var(--dur-fast)]",
        "hover:bg-accent",
        active && "bg-selected before:absolute before:inset-y-[var(--sel-bar-inset)] before:left-0 before:w-[var(--sel-bar-w)] before:rounded-full before:bg-selected-edge before:content-['']",
      )}
      data-clip-row-density="history"
      // Android image rows follow the available width and grow naturally. On
      // desktop the fixed cap keeps the palette's list geometry stable.
      style={image && android
        ? { minHeight: imageRowHeight(rowPreviewLines) }
        : { height: image ? imageRowHeight(rowPreviewLines) : rowHeight(rowPreviewLines) }}
    >
      <span
        aria-label={type.label}
        role="img"
        title={type.label}
        className={cn(
          "mt-px flex size-[var(--icon-lg)] shrink-0 items-center justify-center",
          KIND_TEXT_CLASS[kindOf(item)],
        )}
      >
        {item.is_sensitive ? <ShieldAlert size={14} /> : <TypeIcon size={14} strokeWidth={1.75} />}
      </span>
      <div className="flex min-w-0 flex-1 flex-col items-start">
        <button
          type="button"
          tabIndex={-1}
          onClick={onCopy}
          aria-label={`${t("quickPaste.row.copyPrefix")} ${image ? t("quickPaste.row.image") : text}`}
          className="flex min-w-0 self-stretch flex-col items-start rounded-sm text-left outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
        >
        {image ? (
          <HistoryImagePreview
            id={item.id}
            className="max-w-full self-start"
            style={android ? { maxWidth: "100%" } : { maxHeight: imageHeight }}
          />
        ) : (
          <span
            className={cn(
              "min-w-0 overflow-hidden text-sm leading-normal break-words whitespace-pre-wrap text-foreground",
              item.is_sensitive && "text-withheld-fg",
            )}
            style={{
              display: "-webkit-box",
              WebkitBoxOrient: "vertical",
              WebkitLineClamp: previewLines,
            }}
          >
            {text}
          </span>
        )}
        </button>
        <span className="mt-2 flex h-[18px] min-w-0 items-center gap-2 overflow-hidden whitespace-nowrap text-xs text-muted-foreground">
          <span
            className={cn(
              "flex min-w-0 items-center gap-1 rounded-full bg-elevated px-1.5 py-px",
              !source.available && "text-faint",
            )}
            title={source.label}
          >
            <SourceAppIcon
              bundleId={item.source_app_bundle_id}
              Fallback={SourceIcon}
              className="size-4"
            />
            <span className="truncate">{source.label}</span>
          </span>
          <TimestampToggle createdAt={item.created_at} />
          {item.pinned && (
            <Pin size={11} aria-label={t("quickPaste.row.pinned")} className="text-brand-2" />
          )}
          {potentiallySensitive && (
            <span
              role="img"
              aria-label={t("quickPaste.row.potentialSensitive")}
              title={t("quickPaste.row.potentialSensitive")}
              className="flex shrink-0 items-center text-destructive"
            >
              <ShieldAlert size={12} aria-hidden="true" />
            </span>
          )}
        </span>
      </div>
      {shortcut !== null && (
        <kbd
          aria-hidden="true"
          className="hidden shrink-0 self-center rounded-sm border border-border bg-elevated px-1 py-0.5 font-sans text-[10px] leading-none text-muted-foreground sm:inline"
        >
          {shortcut}
        </kbd>
      )}
      <button
        type="button"
        aria-label={t(item.pinned ? "quickPaste.row.unpin" : "quickPaste.row.pin")}
        title={t(item.pinned ? "quickPaste.row.unpin" : "quickPaste.row.pin")}
        disabled={pinPending}
        onClick={onTogglePin}
        className="flex size-[var(--sz-iconbtn)] shrink-0 items-center justify-center rounded-md text-muted-foreground outline-none transition-colors hover:bg-secondary hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring disabled:opacity-50"
      >
        {item.pinned ? <PinOff size={15} aria-hidden="true" /> : <Pin size={15} aria-hidden="true" />}
      </button>
    </div>
  );
}
