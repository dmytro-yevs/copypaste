import { AlertTriangle, Eye, Pin, Trash2 } from "lucide-react";

// Icon-button primitive (design.md Decision 3's `.iconbtn` family, task 6.6).
// Mirrors the exact classes/markup HistoryRow's row__right cluster uses.
export function IconButtonsSection() {
  return (
    <section id="gallery-iconbuttons">
      <h2>Icon buttons</h2>
      <div className="gallery__row">
        <button type="button" className="iconbtn" aria-label="Preview" title="Preview">
          <Eye aria-hidden="true" />
        </button>
        <button type="button" className="iconbtn star-btn" aria-label="Pin" title="Pin">
          <Pin aria-hidden="true" />
        </button>
        <button type="button" className="iconbtn star-btn on" aria-label="Unpin" title="Unpin">
          <Pin aria-hidden="true" />
        </button>
        <button type="button" className="iconbtn danger" aria-label="Delete" title="Delete">
          <Trash2 aria-hidden="true" />
        </button>
        <span className="iconbtn txt-warn" aria-label="Too large to sync" title="Too large to sync">
          <AlertTriangle aria-hidden="true" />
        </span>
      </div>
    </section>
  );
}
