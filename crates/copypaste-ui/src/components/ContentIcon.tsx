/**
 * ContentIcon + KindChip + ContentIconTile — shared content-type icon
 * components (CopyPaste-tsb, CopyPaste-5917).
 *
 * Unifies the two divergent content-type icon implementations that previously
 * lived in Popup.tsx (ContentChip) and HistoryView.tsx (ContentIcon + KindChip).
 * Per-view agents (History/Popup) will migrate to import from here.
 *
 * ContentIcon      — the type glyph only (lucide-react, stroke 1.5, currentColor)
 * ContentIconTile  — glyph in a square tile (mute/16 bg, spec §ICON-3)
 * KindChip         — the tinted full-word TYPE/URL/IMAGE/CODE pill
 *
 * Icon set: ALL icons come from lucide-react (ICON-11 — single glyph family).
 *
 * Canonical PATH / NUMBER glyphs (ICON-4 — for Android parity):
 *   PATH   → lucide FolderOpen  (Android: Icons.Outlined.FolderOpen — replaces AttachFile)
 *   NUMBER → lucide Hash        (Android: Icons.Outlined.Tag → replace with Hash-equivalent)
 */

import {
  type LucideProps,
  Type,
  Link,
  Image,
  Code,
  FileText,
  Mail,
  Phone,
  Palette,
  Hash,
  FolderOpen,
  Braces,
} from "lucide-react";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Canonical kind values emitted by the daemon's sensitive-detection layer.
 * Covers all distinct content categories the UI needs to handle.
 */
type ContentKind =
  | "text"
  | "url"
  | "image"
  | "code"
  | "email"
  | "phone"
  | "color"
  | "number"
  | "path"
  | "json"
  | "file";

/**
 * Classify a raw content_type string into one of the canonical content kinds.
 * Mirrors the branching used in both Popup.tsx and HistoryView.tsx, extended
 * to cover all distinct daemon-emitted kind labels.
 */
function classify(contentType: string): ContentKind {
  const ct = contentType.toLowerCase();
  if (ct === "text" || ct === "text/plain") return "text";
  if (ct === "url") return "url";
  // image/* (including bare "image") → image
  if (ct === "image" || ct.startsWith("image/")) return "image";
  // code, text/x-*, application/* → code (matches Popup.tsx ContentChip logic)
  if (ct === "code" || ct.startsWith("text/x-") || ct.startsWith("application/"))
    return "code";
  // Daemon-emitted kind labels (lowercase content_type strings)
  if (ct === "email") return "email";
  if (ct === "phone") return "phone";
  if (ct === "color") return "color";
  if (ct === "number") return "number";
  if (ct === "path") return "path";
  if (ct === "json") return "json";
  if (ct === "file") return "file";
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
 *   text   → <Type>       text-ide-faint
 *   url    → <Link>       text-ide-info
 *   image  → <Image>      text-ide-violet
 *   code   → <Code>       text-ide-violet
 *   email  → <Mail>       text-ide-success
 *   phone  → <Phone>      text-ide-success
 *   color  → <Palette>    text-ide-warning
 *   number → <Hash>       text-ide-warning  (canonical: lucide Hash — ICON-4)
 *   path   → <FolderOpen> text-ide-warning  (canonical: lucide FolderOpen — ICON-4)
 *   json   → <Braces>     text-ide-danger
 *   file   → <FileText>   text-ide-dim
 *
 * All icons: strokeWidth=1.5, aria-hidden=true. All from lucide-react (ICON-11).
 */
export function ContentIcon({ contentType, size = 14 }: ContentIconProps) {
  const kind = classify(contentType);

  const shared: LucideProps = {
    width: size,
    height: size,
    strokeWidth: 1.5,
    "aria-hidden": true,
  };

  switch (kind) {
    case "text":
      // 5917.80: TEXT → faint (grey), not accent (blue); matches KindChip fallback + Android c.faint
      return <Type {...shared} className="shrink-0 text-ide-faint" />;
    case "url":
      // crh3.42: PARITY-SPEC §6 canonical URL token = ide-info (teal).
      // Reverts 1hqt (sky), which deviated from spec; Android uses c.info.
      return <Link {...shared} className="shrink-0 text-ide-info" />;
    case "image":
      // 1jms.14: IMAGE → violet per PARITY-SPEC §6 (distinct from URL=sky; matches Android c.violet)
      return <Image {...shared} className="shrink-0 text-ide-violet" />;
    case "code":
      return <Code {...shared} className="shrink-0 text-ide-violet" />;
    case "email":
      return <Mail {...shared} className="shrink-0 text-ide-success" />;
    case "phone":
      return <Phone {...shared} className="shrink-0 text-ide-success" />;
    case "color":
      // Live color swatch is rendered separately in history rows; this glyph
      // represents the kind in compact contexts (chips, popup rows).
      return <Palette {...shared} className="shrink-0 text-ide-warning" />;
    case "number":
      return <Hash {...shared} className="shrink-0 text-ide-warning" />;
    case "path":
      return <FolderOpen {...shared} className="shrink-0 text-ide-warning" />;
    case "json":
      return <Braces {...shared} className="shrink-0 text-ide-danger" />;
    case "file":
      return <FileText {...shared} className="shrink-0 text-ide-dim" />;
    default: {
      // Exhaustive type narrowing: TypeScript knows `kind` is `never` here.
      const _exhaustive: never = kind;
      void _exhaustive;
      return <FileText {...shared} className="shrink-0 text-ide-dim" />;
    }
  }
}

// ---------------------------------------------------------------------------
// ContentIconTile — glyph inside a rounded square tile (ICON-3)
// ---------------------------------------------------------------------------

export interface ContentIconTileProps extends ContentIconProps {
  /**
   * Tile size in px (the square container). Default: 26.
   * The glyph size is derived from the icon's own `size` prop; tile padding
   * centres it. Override `size` separately when you need a non-default glyph.
   */
  tileSize?: number;
  /** Extra class names forwarded to the tile wrapper. */
  className?: string;
  /** Forwarded to the tile wrapper span for accessibility. */
  "aria-label"?: string;
  /** Forwarded to the tile wrapper span for tooltip. */
  title?: string;
  /** Forwarded to the tile wrapper span (e.g. role="img"). */
  role?: string;
}

/**
 * A square tile that wraps <ContentIcon> with the spec-required mute/16
 * background (ICON-3: spec requires mute/16, NOT faint/16).
 *
 * Usage in HistoryView rows (migrate from the inline bg-ide-faint/16 div):
 *   <ContentIconTile contentType={entry.content_type} />
 */
export function ContentIconTile({
  contentType,
  size = 14,
  tileSize = 26,
  className,
  "aria-label": ariaLabel,
  title,
  role,
}: ContentIconTileProps) {
  return (
    <span
      className={[
        "flex shrink-0 items-center justify-center rounded-[7px]",
        // ICON-3: tile background must be mute/16, NOT faint/16.
        "bg-ide-mute/16",
        className ?? "",
      ]
        .join(" ")
        .trim()}
      style={{ width: tileSize, height: tileSize }}
      aria-label={ariaLabel}
      title={title}
      role={role}
    >
      <ContentIcon contentType={contentType} size={size} />
    </span>
  );
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
 *
 * Exported so callers (e.g. HistoryView) can share this logic without
 * duplicating the content-type → label mapping (CopyPaste-bdac.29).
 */
export function kindFallback(contentType: string): string {
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
 * Canonical kind→color table (spec §6, ICON-2 update):
 *   TEXT                        → faint (grey) — ICON-2: was accent/blue; spec .b-text wants faint
 *   URL                         → info (teal) — PARITY-SPEC §6; crh3.42 reverts 1hqt sky
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
  // crh3.42: URL uses ide-info token (PARITY-SPEC §6); 1hqt sky reverted.
  // lplk: violet is now 128 90 213 (AA-darkened via CSS var)
  // 1jms.14: IMAGE → violet per PARITY-SPEC §6 (distinct from URL=info; matches Android c.violet)
  const colorClass =
    label === "URL"
      ? "text-ide-info border-ide-info/45 bg-ide-info/14"
      : label === "EMAIL" || label === "PHONE"
      ? "text-ide-success border-ide-success/45 bg-ide-success/14"
      : label === "COLOR" || label === "NUMBER" || label === "PATH"
      ? "text-ide-warning border-ide-warning/45 bg-ide-warning/14"
      : label === "JSON" || label === "PRIVATE" || label === "SENSITIVE"
      ? "text-ide-danger border-ide-danger/45 bg-ide-danger/14"
      : label === "IMAGE"
      ? "text-ide-violet border-ide-violet/45 bg-ide-violet/14"
      : label === "CODE"
      ? "text-ide-violet border-ide-violet/45 bg-ide-violet/14"
      : label === "FILE"
      ? "text-ide-dim border-ide-dim/45 bg-ide-dim/14"
      : /* TEXT / fallback — ICON-2: faint/grey, not accent/blue (spec .b-text) */
        "text-ide-faint border-ide-faint/45 bg-ide-faint/14";

  return (
    <span
      className={[
        "flex shrink-0 items-center border px-1 py-px",
        // audit P1-3: bumped 9px → 10px for legibility.
        "text-[10.5px] font-semibold leading-none tracking-wide uppercase",
        colorClass,
      ].join(" ")}
      aria-label={`Type: ${label}`}
      style={{ borderRadius: "var(--r-chip)" }}
    >
      {label}
    </span>
  );
}
