import { ContentTile } from "../../../lib/clip/ContentTile";
import { normalizeContentKind } from "../../../lib/clip/normalizeContentKind";
import { KIND_PRESENTATION } from "../../../lib/clip/kindPresentation";

// The 11 raw daemon `kind` values (HistoryEntry.kind's documented union —
// lib/ipc/types.ts) — task 6.6 "tiles (all 11 kinds + unknown)". Several
// collapse onto the same NormalizedKind/tile presentation (PHONE/NUMBER →
// num, PATH/FILE → file — design.md Decision 8) — showing all 11 raw values
// demonstrates that aliasing explicitly, not just the 9 distinct tiles it
// produces.
const RAW_KINDS = [
  "TEXT",
  "URL",
  "EMAIL",
  "PHONE",
  "NUMBER",
  "COLOR",
  "JSON",
  "CODE",
  "PATH",
  "FILE",
  "IMAGE",
] as const;

export function TilesSection() {
  return (
    <section id="gallery-tiles">
      <h2>Content-type tiles — all 11 daemon kinds + unknown fallback</h2>
      <div className="gallery__row">
        {RAW_KINDS.map((raw) => {
          const kind = normalizeContentKind({ kind: raw });
          return (
            <figure key={raw} className="gallery__tile-example">
              <ContentTile kind={kind} colorValue={kind === "color" ? "#6C47FF" : undefined} />
              <figcaption>
                {raw} → {KIND_PRESENTATION[kind].label}
              </figcaption>
            </figure>
          );
        })}
        {/* Unknown fallback — a future/unrecognized kind string. */}
        <figure className="gallery__tile-example">
          <ContentTile kind="unknown" />
          <figcaption>(unrecognized) → {KIND_PRESENTATION.unknown.label}</figcaption>
        </figure>
      </div>
    </section>
  );
}
