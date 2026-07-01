import { Eye } from "lucide-react";

// ---------------------------------------------------------------------------
// Forced-state testability (task 6.8, design.md Decision 7/G1).
//
// Native pseudo-states (:hover/:active/:focus-visible) can't be persistently
// rendered as a static screenshot fixture. `data-force-state="hover"|"active"|
// "focus"` on this representative set is the debug-only mechanism that lets a
// static page still show what the real pseudo-class looks like — gallery.css
// mirrors each real rule's declarations under a `[data-force-state=…]`
// selector, and forcedState.parity.test.ts asserts the two never drift.
// ---------------------------------------------------------------------------
export function ForcedStateSection() {
  return (
    <section id="gallery-forced-states">
      <h2>Forced states (debug-only)</h2>
      <p className="gallery__note">
        Each example below is NOT actually hovered/pressed/focused — the
        <code> data-force-state</code> attribute forces the same computed
        styles as the real pseudo-class (see gallery.css).
      </p>
      <div className="gallery__row">
        <button type="button" className="btn btn--primary" data-force-state="hover">
          Primary · hover
        </button>
        <button type="button" className="btn btn--secondary" data-force-state="hover">
          Secondary · hover
        </button>
        <button type="button" className="btn btn--primary" data-force-state="active">
          Primary · active
        </button>
        <button type="button" className="iconbtn" data-force-state="hover" aria-label="Icon hover">
          <Eye aria-hidden="true" />
        </button>
        <button type="button" className="iconbtn" data-force-state="active" aria-label="Icon active">
          <Eye aria-hidden="true" />
        </button>
        <span className="chip" data-force-state="hover">
          Chip · hover
        </span>
        <button type="button" className="btn btn--secondary" data-force-state="focus">
          Focus ring
        </button>
      </div>
      <div className="list" role="listbox" aria-label="Row hover example">
        <div className="row" data-force-state="hover">
          <div className="row__body">
            <div className="row__title">Row · hover</div>
          </div>
        </div>
      </div>
    </section>
  );
}
