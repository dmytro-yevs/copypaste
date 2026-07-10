/**
 * BulkActionBar — shown when ≥1 item is multi-selected in HistoryView.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 */
// CopyPaste-bdac.23: ActionButton replaces raw <button> elements in BulkActionBar.
import { ActionButton } from "../../components/ActionButton";

export interface BulkBarProps {
  count: number;
  allSelected: boolean;
  /**
   * CopyPaste-8ebg.55: true when every currently multi-selected item is
   * already pinned. Drives the single Pin/Unpin toggle below — previously
   * Pin and Unpin were both always shown even though one is always a no-op
   * for the current selection (e.g. Unpin when nothing selected is pinned).
   */
  allPinned: boolean;
  onSelectAll: () => void;
  onClearSelection: () => void;
  onBulkCopy: () => void;
  onBulkPin: () => void;
  onBulkUnpin: () => void;
  onBulkDelete: () => void;
  isBusy: boolean;
}

export function BulkActionBar({
  count,
  allSelected,
  allPinned,
  onSelectAll,
  onClearSelection,
  onBulkCopy,
  onBulkPin,
  onBulkUnpin,
  onBulkDelete,
  isBusy,
}: BulkBarProps) {
  return (
    <div className="bulkbar show">
      {/* Selection count — neutral text, no amber */}
      <span className="bulkbar__n">
        {count} selected
      </span>

      <span>|</span>

      {/* Select-all toggle — CopyPaste-bdac.23: ActionButton(secondary,sm).
          CopyPaste-5917.18: aria-pressed conveys toggle state to screen readers. */}
      <ActionButton
        variant="secondary"
        size="sm"
        aria-pressed={allSelected}
        onClick={allSelected ? onClearSelection : onSelectAll}
        disabled={isBusy}
      >
        {allSelected ? "Deselect all" : "Select all"}
      </ActionButton>

      {/* Bulk actions — CopyPaste-bdac.23: ActionButton replaces raw <button>. */}
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onBulkCopy}
        disabled={isBusy}
        title="Copy selected items (concatenated with newlines)"
        aria-label="Copy selected items"
      >
        Copy
      </ActionButton>
      {/* CopyPaste-8ebg.55: single toggle reflecting the selection's pin state
          instead of always showing both Pin and Unpin (one of which is always
          a no-op for the current selection). */}
      <ActionButton
        variant="secondary"
        size="sm"
        aria-pressed={allPinned}
        onClick={allPinned ? onBulkUnpin : onBulkPin}
        disabled={isBusy}
      >
        {allPinned ? "Unpin" : "Pin"}
      </ActionButton>
      <ActionButton
        variant="danger"
        size="sm"
        onClick={onBulkDelete}
        disabled={isBusy}
      >
        Delete
      </ActionButton>

      {/* Spacer */}
      <span />

      {/* Clear selection — CopyPaste-bdac.23: ActionButton(secondary,sm). */}
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onClearSelection}
        disabled={isBusy}
        title="Clear selection (Escape)"
      >
        Clear
      </ActionButton>
    </div>
  );
}
