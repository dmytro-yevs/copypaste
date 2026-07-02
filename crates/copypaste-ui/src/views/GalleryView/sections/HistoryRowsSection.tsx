import { HistoryRow } from "../../HistoryView/HistoryRow";
import { makeHistoryEntry } from "../../../lib/fixtures";
import type { HistoryEntry } from "../../../lib/ipc";
import { galleryRowDefaults } from "./rowDefaults";

// Task 6.7: "history row (one per content kind + unknown + one long-text
// example)". One HistoryEntry per NormalizedKind (design.md Decision 8) that
// HistoryRow's ContentTile/ClipMetadata presentation branches on, plus an
// explicit unknown-kind row and a pathologically long single-line preview.
const ROWS: HistoryEntry[] = [
  makeHistoryEntry({
    id: "gallery-hrow-text",
    kind: "TEXT",
    content_type: "text",
    preview: "Plain clipboard text example.",
  }),
  makeHistoryEntry({
    id: "gallery-hrow-url",
    kind: "URL",
    content_type: "text",
    preview: "https://copypaste.app/docs/getting-started",
  }),
  makeHistoryEntry({
    id: "gallery-hrow-mail",
    kind: "EMAIL",
    content_type: "text",
    preview: "team@copypaste.app",
  }),
  makeHistoryEntry({
    id: "gallery-hrow-num",
    kind: "NUMBER",
    content_type: "text",
    preview: "3.14159265",
  }),
  makeHistoryEntry({
    id: "gallery-hrow-color",
    kind: "COLOR",
    content_type: "text",
    preview: "#22C55E",
  }),
  makeHistoryEntry({
    id: "gallery-hrow-json",
    kind: "JSON",
    content_type: "text",
    preview: '{"ok":true,"count":3}',
  }),
  makeHistoryEntry({
    id: "gallery-hrow-code",
    kind: "CODE",
    content_type: "text",
    preview: "const x: number = 1;",
  }),
  makeHistoryEntry({
    id: "gallery-hrow-file",
    kind: "FILE",
    content_type: "file",
    preview: "[file: report.pdf]",
  }),
  makeHistoryEntry({
    id: "gallery-hrow-image",
    kind: "IMAGE",
    content_type: "image/png",
    preview: "[image]",
  }),
  // Unknown — a kind the daemon might send that this build doesn't recognize.
  makeHistoryEntry({
    id: "gallery-hrow-unknown",
    kind: "AUDIO",
    content_type: "audio/mp3",
    preview: "voice-memo.m4a",
  }),
  // Long-text example — stress-tests row__title's ellipsis/overflow handling.
  makeHistoryEntry({
    id: "gallery-hrow-long",
    kind: "TEXT",
    preview:
      "This is a deliberately long single-line clipboard entry used to prove row__title truncates with an ellipsis instead of wrapping or overflowing the row — " +
      "the quick brown fox jumps over the lazy dog, repeatedly, until the string is long enough to exceed any reasonable row width.",
  }),
];

export function HistoryRowsSection() {
  return (
    <section id="gallery-history-row">
      <h2>History row — one per content kind + unknown + long text</h2>
      <div className="list" role="listbox" aria-label="History row examples">
        {ROWS.map((entry) => (
          <HistoryRow key={entry.id} entry={entry} {...galleryRowDefaults} />
        ))}
      </div>
    </section>
  );
}
