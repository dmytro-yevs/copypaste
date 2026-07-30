/**
 * The bulk action bar (manifest §3.1.9).
 *
 * * **Pin is one toggle, not two buttons.** Its label reflects whether *every*
 *   selected item is already pinned, so it always says what pressing it will
 *   do (CopyPaste-8ebg.55).
 * * **Bulk delete confirms.** There is no undo for it, unlike the single-row
 *   delete's five-second window, so the caller's dialog is the only gate.
 *
 * It sits above the list rather than floating over it: a floating bar covers
 * the rows the user is choosing between, and on a phone it lands exactly where
 * the thumb is.
 */
import { Pin, PinOff, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTranslation } from "@/i18n";

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
  const { t } = useTranslation();
  const nothing = count === 0;

  return (
    <div
      // A region rather than a toolbar: it is a labelled section of the page
      // whose controls are ordinary buttons, and `role="toolbar"` would take
      // over arrow-key handling that belongs to the list below it.
      role="region"
      aria-label={t("history.bulk.label")}
      className="flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider bg-raised-1 px-s-3 py-s-2"
    >
      <span className="text-xs tabular-nums text-foreground" aria-live="polite">
        {nothing
          ? t("history.bulk.empty")
          : t("history.bulk.selected", { count })}
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
          {t(allPinned ? "history.bulk.unpin" : "history.bulk.pin")}
        </Button>

        <Button
          variant="outline"
          size="sm"
          disabled={nothing || busy}
          className="text-err-strong"
          onClick={onDelete}
        >
          <Trash2 aria-hidden="true" />
          {t("history.bulk.delete")}
        </Button>

        <Button variant="ghost" size="sm" onClick={onCancel}>
          <X aria-hidden="true" />
          {t("history.bulk.done")}
        </Button>
      </div>
    </div>
  );
}
