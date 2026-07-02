// ---------------------------------------------------------------------------
// Shared clipboard content-kind normalization (design.md Decision 8 / task 3.2).
// Used by BOTH HistoryRow and PopupRow so kind interpretation is defined once
// and cannot drift between the two. Every input — including undefined, an
// unrecognized string, or a future daemon kind — normalizes to a known member
// (never a runtime error or a silently blank tile).
// ---------------------------------------------------------------------------

/** The closed set of normalized content kinds (plus `unknown`). */
export type NormalizedKind =
  | "text"
  | "url"
  | "mail"
  | "num"
  | "color"
  | "json"
  | "code"
  | "file"
  | "image"
  | "unknown";

/** Just the fields normalization reads — accepts any `HistoryEntry`-shaped object. */
export interface KindSource {
  kind?: string;
  content_type?: string;
}

// Daemon `kind` values → normalized kind. PATH/FILE share `file`; PHONE/NUMBER
// share `num` (design.md Decision 8). Matching is case-insensitive (uppercased
// before lookup).
const KIND_ALIASES: Record<string, NormalizedKind> = {
  TEXT: "text",
  URL: "url",
  EMAIL: "mail",
  PHONE: "num",
  NUMBER: "num",
  COLOR: "color",
  JSON: "json",
  CODE: "code",
  PATH: "file",
  FILE: "file",
  IMAGE: "image",
};

function isImageMime(contentType: string | undefined): boolean {
  return (
    typeof contentType === "string" &&
    contentType.trim().toLowerCase().startsWith("image/")
  );
}

/**
 * Normalize a clipboard entry to a {@link NormalizedKind}.
 *
 * Precedence: the daemon's refined `kind` wins over `content_type` when present
 * and recognized — EXCEPT the image case, where an image MIME `content_type`
 * wins even if `kind` is absent or contradictory (MIME is the more reliable
 * signal for images). Anything unrecognized (or `undefined`) → `"unknown"`.
 */
export function normalizeContentKind(entry: KindSource): NormalizedKind {
  const rawKind =
    typeof entry.kind === "string" && entry.kind.trim() !== ""
      ? entry.kind.trim().toUpperCase()
      : undefined;
  const mapped = rawKind ? KIND_ALIASES[rawKind] : undefined;

  // Image MIME wins whenever kind is absent OR contradictory (i.e. does not
  // already resolve to "image").
  if (isImageMime(entry.content_type) && mapped !== "image") {
    return "image";
  }

  if (mapped) return mapped;

  // kind absent or unrecognized (including a future hypothetical kind) → derive
  // conservatively from content_type; unknown when nothing matches.
  const ct =
    typeof entry.content_type === "string"
      ? entry.content_type.trim().toLowerCase()
      : "";
  if (ct === "file") return "file";
  if (ct === "text" || ct.startsWith("text/")) return "text";
  return "unknown";
}
