/**
 * ContentIcon + KindChip — shared content-type icon components (CopyPaste-tsb).
 *
 * Unifies the two divergent content-type icon implementations that previously
 * lived in Popup.tsx (ContentChip) and HistoryView.tsx (ContentIcon + KindChip).
 * Per-view agents (History/Popup) will migrate to import from here.
 *
 * ContentIcon  — the type glyph only (lucide-react, stroke 1.5, currentColor)
 * KindChip     — the tinted full-word TYPE/URL/IMAGE/CODE pill
 */

import { type LucideProps, Type, Link, Image, Code, FileText } from "lucide-react";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Classify a raw content_type string into one of four canonical categories.
 * Mirrors the branching used in both Popup.tsx and HistoryView.tsx so the
 * shared component produces identical output.
 */
function classify(contentType: string): "text" | "url" | "image" | "code" {
  if (contentType === "text" || contentType === "text/plain") return "text";
  if (contentType === "url") return "url";
  // image/* (including bare "image") → image
  if (contentType === "image" || contentType.startsWith("image/")) return "image";
  // code, text/x-*, application/* → code (matches Popup.tsx ContentChip logic)
  if (
    contentType === "code" ||
    contentType.startsWith("text/x-") ||
    contentType.startsWith("application/")
  )
    return "code";
  // Everything else (plain "text/*" other than text/plain, file, etc.) → text fallback
  return "text";
}

// ---------------------------------------------------------------------------
// ContentIcon — the SVG glyph only
// ---------------------------------------------------------------------------

export interface ContentIconProps {
  /** Raw content_type string (e.g. "text", "url", "image/png", "text/x-python"). */
  contentType: string;
  /** Icon size in px. Default: 14. */
  size?: number;
}

/**
 * Renders a lucide-react icon tinted with the appropriate IDE design-token
 * color class based on the content type category:
 *
 *   text  → <Type>    text-ide-accent
 *   url   → <Link>    text-ide-info
 *   image → <Image>   text-ide-violet
 *   code  → <Code>    text-ide-violet
 *
 * All icons: strokeWidth=1.5, aria-hidden=true.
 */
export function ContentIcon({ contentType, size = 14 }: ContentIconProps) {
  const category = classify(contentType);

  const shared: LucideProps = {
    width: size,
    height: size,
    strokeWidth: 1.5,
    "aria-hidden": true,
  };

  switch (category) {
    case "text":
      return (
        <Type
          {...shared}
          className="shrink-0 text-ide-accent"
        />
      );
    case "url":
      return (
        <Link
          {...shared}
          className="shrink-0 text-ide-info"
        />
      );
    case "image":
      return (
        <Image
          {...shared}
          className="shrink-0 text-ide-violet"
        />
      );
    case "code":
      return (
        <Code
          {...shared}
          className="shrink-0 text-ide-violet"
        />
      );
    default: {
      // Exhaustive type narrowing: TypeScript knows `category` is `never` here.
      const _exhaustive: never = category;
      void _exhaustive;
      return (
        <FileText
          {...shared}
          className="shrink-0 text-ide-faint"
        />
      );
    }
  }
}

// ---------------------------------------------------------------------------
// KindChip — the tinted full-word pill
// ---------------------------------------------------------------------------

export interface KindChipProps {
  /** Raw content_type string (e.g. "text", "url", "image"). */
  contentType: string;
  /**
   * Daemon-computed kind label (e.g. "URL", "EMAIL", "CODE").
   * When provided, takes precedence over the contentType-derived label.
   */
  kind?: string;
}

/**
 * Derive a fallback kind label from content_type when the daemon does not
 * emit `kind`. Mirrors the daemon's fixed labels for UI consistency.
 */
function kindFallback(contentType: string): string {
  if (contentType === "url") return "URL";
  if (contentType === "image" || contentType.startsWith("image/")) return "IMAGE";
  if (
    contentType === "code" ||
    contentType.startsWith("text/x-") ||
    contentType.startsWith("application/")
  )
    return "CODE";
  return "TEXT";
}

/**
 * Full-word type chip with semantic IDE token colors.
 *
 * Color mapping (matches HistoryView.tsx KindChip exactly):
 *   URL            → text-ide-info
 *   EMAIL / PHONE  → text-ide-success
 *   COLOR / NUMBER / PATH → text-ide-warning
 *   JSON           → text-ide-danger
 *   CODE / IMAGE   → text-ide-violet
 *   TEXT / unknown → text-ide-accent
 */
export function KindChip({ contentType, kind }: KindChipProps) {
  const label = kind ?? kindFallback(contentType);

  const colorClass =
    label === "URL"
      ? "text-ide-info border-ide-info/40 bg-ide-info/8"
      : label === "EMAIL" || label === "PHONE"
      ? "text-ide-success border-ide-success/40 bg-ide-success/8"
      : label === "COLOR" || label === "NUMBER" || label === "PATH"
      ? "text-ide-warning border-ide-warning/40 bg-ide-warning/8"
      : label === "JSON"
      ? "text-ide-danger border-ide-danger/40 bg-ide-danger/8"
      : label === "CODE" || label === "IMAGE"
      ? "text-ide-violet border-ide-violet/40 bg-ide-violet/8"
      : /* TEXT / fallback */ "text-ide-accent border-ide-accent/40 bg-ide-accent/8";

  return (
    <span
      className={[
        "flex shrink-0 items-center rounded border px-1 py-px",
        "text-[9px] font-semibold leading-none tracking-wide uppercase",
        colorClass,
      ].join(" ")}
      aria-label={`Type: ${label}`}
    >
      {label}
    </span>
  );
}
