/**
 * The bulk action bar (manifest §3.1.9).
 *
 * Two rules it exists to satisfy:
 *
 * * **Pin is one toggle, not two buttons.** Its label reflects whether *every*
 *   selected item is already pinned, so it always says what pressing it will
 *   do (CopyPaste-8ebg.55).
 * * **Bulk delete confirms.** There is no undo for a bulk delete, unlike the
 *   single-row delete's five-second window, so the dialog is the only gate.
 *   The caller owns the dialog; this only asks for it.
 *
 * It sits above the list rather than floating over it: a floating bar covers
 * the rows the user is choosing between, and on a phone it lands exactly where
 * the thumb is.
 */
import { Pin, PinOff, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";

interface BulkBarProps {
  count: number;
  allPinned: boolean;
  busy: boolean;
  onTogglePin: () => void;
  onDelete: () => void;
  onCancel: () => void;
}

export function BulkBar({
  count,
  allPinned,
  busy,
  onTogglePin,
  onDelete,
  onCancel,
}: BulkBarProps) {
  const nothing = count === 0;

  return (
    <div
      // A region rather than a toolbar: it is a labelled section of the page
      // whose controls are ordinary buttons, and `role="toolbar"` would take
      // over arrow-key handling that belongs to the list below it.
      role="region"
      aria-label="Selection actions"
      className="flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider bg-raised-1 px-s-3 py-s-2"
    >
      <span className="text-xs tabular-nums text-foreground" aria-live="polite">
        {nothing
          ? "Select items to act on them"
          : `${count} item${count === 1 ? "" : "s"} selected`}
      </span>

      <div className="ml-auto flex items-center gap-s-2">
        <Button
          variant="outline"
          size="sm"
          disabled={nothing || busy}
          onClick={onTogglePin}
        >
          {allPinned ? (
            <PinOff aria-hidden="true" />
          ) : (
            <Pin aria-hidden="true" />
          )}
          {allPinned ? "Unpin" : "Pin"}
        </Button>

        <Button
          variant="outline"
          size="sm"
          disabled={nothing || busy}
          className="text-err-strong"
          onClick={onDelete}
        >
          <Trash2 aria-hidden="true" />
          Delete
        </Button>

        <Button variant="ghost" size="sm" onClick={onCancel}>
          <X aria-hidden="true" />
          Done
        </Button>
      </div>
    </div>
  );
}
