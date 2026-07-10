import { type MouseEvent } from "react";
import type { HistoryEntry } from "../ipc";

/** Accessible placeholder announced (by the consumer) while an item is masked. */
export const MASKED_A11Y_LABEL = "Sensitive item, hidden — activate to reveal";

export interface ClipPreviewProps {
  entry: HistoryEntry;
  /** True when the item is sensitive, masking is on, and it isn't revealed yet. */
  masked: boolean;
  /** Reveal the masked content (click on the mask). */
  onReveal: () => void;
  /** Render the preview in the monospace title variant. */
  mono?: boolean;
}

/**
 * Shared single-line clip preview with the sensitive-masking contract (design.md
 * Decision 9 / X6): VISUAL blur only. The real preview text stays in the DOM
 * (so the mask occupies its real rendered width — no length masking — and stays
 * selectable) but is `aria-hidden`, so its accessible name never leaks. The
 * CONSUMER (the row) carries the masked `aria-label` and reveal semantics for
 * its own role; here the mask is click-to-reveal (mouse). Copy/paste reads from
 * item DATA elsewhere, never from this masked node.
 */
export function ClipPreview({ entry, masked, onReveal, mono }: ClipPreviewProps) {
  const cls = mono ? "row__title mono" : "row__title";

  if (masked) {
    // CopyPaste-8ebg.55: a real <button> instead of a span+onClick — native
    // button semantics give this Tab-stop focus and Enter/Space activation
    // for free, fixing the keyboard-unreachable reveal affordance. The mask
    // stays `aria-hidden` (the accessible name must never leak the plaintext
    // preview — X6), so the button carries its own explicit aria-label.
    const reveal = (e: MouseEvent) => {
      e.stopPropagation();
      onReveal();
    };
    return (
      <div className={cls}>
        <button
          type="button"
          className="mask"
          aria-label="Sensitive content hidden — activate to reveal"
          title="Click to reveal sensitive content"
          onClick={reveal}
        >
          <span aria-hidden="true">{entry.preview}</span>
        </button>
      </div>
    );
  }

  return <div className={cls}>{entry.preview}</div>;
}
