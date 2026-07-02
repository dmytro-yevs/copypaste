/**
 * BulkActionBar — shown when ≥1 item is multi-selected in HistoryView.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 */
// CopyPaste-bdac.23: ActionButton replaces raw <button> elements in BulkActionBar.
import { ActionButton } from "../../components/ActionButton";

export interface BulkBarProps {
  count: number;
  allSelected: boolean;
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
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onBulkPin}
        disabled={isBusy}
      >
        Pin
      </ActionButton>
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onBulkUnpin}
        disabled={isBusy}
      >
        Unpin
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
