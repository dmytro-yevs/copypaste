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
          // 1hqt: URL/IMAGE use sky token (20 120 170 in light, teal in dark)
          className="shrink-0 text-ide-sky"
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
 * Canonical kind→color table (spec §6):
 *   TEXT                        → accent (blue)
 *   URL                         → info (teal)
 *   EMAIL / PHONE               → success (green)
 *   COLOR / NUMBER / PATH       → warning (amber)
 *   JSON                        → danger (red)
 *   CODE / IMAGE                → violet
 *   FILE                        → dim (grey)
 *   PRIVATE / SENSITIVE         → danger (red)
 */
export function KindChip({ contentType, kind }: KindChipProps) {
  const label = kind ?? kindFallback(contentType);

  // audit P1-3: colored text on a bare/8% tint read 2.2–2.5:1. Give every kind a
  // stronger tinted fill (14% over the surface — matches the *Dim tint tokens)
  // and keep the semantic text colour; the heavier fill + the AA-darkened
  // danger/faint tokens lift the badge to AA. (The text colour itself is the
  // "one step darker" semantic token, not a lighter decorative tint.)
  // 1hqt: URL/IMAGE use sky token; lplk: violet is now 128 90 213 (AA-darkened via CSS var)
  const colorClass =
    label === "URL"
      ? "text-ide-sky border-ide-sky/45 bg-ide-sky/14"
      : label === "EMAIL" || label === "PHONE"
      ? "text-ide-success border-ide-success/45 bg-ide-success/14"
      : label === "COLOR" || label === "NUMBER" || label === "PATH"
      ? "text-ide-warning border-ide-warning/45 bg-ide-warning/14"
      : label === "JSON" || label === "PRIVATE" || label === "SENSITIVE"
      ? "text-ide-danger border-ide-danger/45 bg-ide-danger/14"
      : label === "CODE" || label === "IMAGE"
      ? "text-ide-violet border-ide-violet/45 bg-ide-violet/14"
      : label === "FILE"
      ? "text-ide-dim border-ide-dim/45 bg-ide-dim/14"
      : /* TEXT / fallback */ "text-ide-accent border-ide-accent/45 bg-ide-accent/14";

  return (
    <span
      className={[
        // ix8u: rounded-ide-sm = 7px chip radius (styleguide --radius-chip)
        "flex shrink-0 items-center rounded-ide-sm border px-1 py-px",
        // audit P1-3: bumped 9px → 10px for legibility.
        "text-[10px] font-semibold leading-none tracking-wide uppercase",
        colorClass,
      ].join(" ")}
      aria-label={`Type: ${label}`}
    >
      {label}
    </span>
  );
}
