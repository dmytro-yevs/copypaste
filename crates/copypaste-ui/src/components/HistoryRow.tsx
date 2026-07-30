/**
 * One history row.
 *
 * Two rules here are the ones v1 paid for:
 *
 *  - **A sensitive item never renders its content** (INV-10, CLAUDE.md rule 4).
 *    The plaintext is not merely blurred, it is absent from the DOM: a masked
 *    row shows a placeholder, and the row's accessible name is a fixed string
 *    with no substring of the preview in it (AT-13). Revealing is an explicit
 *    act, and re-hides on blur and after 10s (INV-11, in useReveal).
 *  - The row is laid out inside the box the virtualiser reserved and clips to
 *    it (`h-full overflow-hidden`), so a long clip can never overlap its
 *    neighbour regardless of pane width (INV-5).
 *
 * Rows are `role="listitem"`, never `role="option"`: option is
 * `childrenPresentational` and would flatten the copy/pin/delete buttons into
 * an axe `nested-interactive` violation (INV-8). Selection is exposed with
 * `aria-current` and announced by the list's sibling live region (INV-9).
 */
import { memo } from "react";
import { Copy, Eye, EyeOff, Pin, PinOff, ShieldAlert, Trash2 } from "lucide-react";

import { cn } from "../lib/cn";
import {
  KIND_TEXT_CLASS,
  MONO_KINDS,
  absoluteTime,
  kindOf,
  previewOf,
  shortAge,
} from "../lib/format";
import type { Item } from "../lib/ipc";
import { PREVIEW_LINES } from "../lib/layout";
import { IconButton } from "./IconButton";

/** INV-10 / A11Y-3 — exact strings from manifest 06. */
export const SENSITIVE_A11Y_LABEL = "Sensitive item, hidden — activate to reveal";
export const SENSITIVE_REVEAL_LABEL =
  "Sensitive content hidden — activate to reveal";
const SENSITIVE_PLACEHOLDER = "Sensitive content hidden";

/**
 * The row's accessible name, and the string the live region mirrors.
 * Deliberately free of the relative age: the age changes on its own, and a
 * live region that re-announces every minute is worse than one that announces
 * only on selection changes (INV-9).
 */
export function rowLabel(item: Item): string {
  const body = item.is_sensitive ? SENSITIVE_A11Y_LABEL : previewOf(item.content);
  return item.pinned ? `Pinned. ${body}` : body;
}

interface HistoryRowProps {
  item: Item;
  active: boolean;
  revealed: boolean;
  flashing: boolean;
  onActivate: (item: Item) => void;
  onCopy: (item: Item) => void;
  onTogglePin: (item: Item) => void;
  onDelete: (item: Item) => void;
  onReveal: (item: Item) => void;
  onHide: () => void;
}

function HistoryRowImpl({
  item,
  active,
  revealed,
  flashing,
  onActivate,
  onCopy,
  onTogglePin,
  onDelete,
  onReveal,
  onHide,
}: HistoryRowProps) {
  const kind = kindOf(item);
  const masked = item.is_sensitive && !revealed;
  // Action buttons are only in the tab order for the active row — a roving
  // tabindex, so 200 rows do not mean 600 tab stops.
  const actionTab = active ? 0 : -1;

  return (
    <div
      className={cn(
        "group relative flex h-full items-start gap-[var(--gap-row)] overflow-hidden rounded-row border border-transparent px-[var(--pad-row-x)] py-[var(--pad-row-y)] transition-colors duration-[var(--dur-fast)]",
        "hover:bg-hover",
        active && "border-[var(--border)] bg-selected",
        flashing && "bg-selected",
      )}
      onClick={() => onActivate(item)}
    >
      <span
        aria-hidden="true"
        className={cn(
          "mt-[1px] flex size-[var(--icon-lg)] shrink-0 items-center justify-center",
          KIND_TEXT_CLASS[kind],
        )}
      >
        {item.is_sensitive ? (
          <ShieldAlert size={14} />
        ) : (
          <Copy size={14} strokeWidth={1.75} />
        )}
      </span>

      <div className="flex min-w-0 flex-1 flex-col">
        {masked ? (
          <button
            type="button"
            aria-label={SENSITIVE_REVEAL_LABEL}
            title="Reveal sensitive content"
            tabIndex={actionTab}
            onClick={(event) => {
              event.stopPropagation();
              onReveal(item);
            }}
            className="flex w-fit items-center gap-[var(--gap-badge)] rounded-sm text-fs-md text-c-secret"
          >
            <span
              className="h-[var(--fs-md)] w-[80px] rounded-xs bg-[currentColor] opacity-25"
              aria-hidden="true"
            />
            <span>{SENSITIVE_PLACEHOLDER}</span>
            <Eye size={12} aria-hidden="true" />
          </button>
        ) : (
          <p
            className={cn(
              "min-w-0 overflow-hidden text-fs-base leading-normal break-words whitespace-pre-wrap text-text",
              MONO_KINDS.has(kind) && "font-mono text-fs-125",
            )}
            style={{
              display: "-webkit-box",
              WebkitBoxOrient: "vertical",
              WebkitLineClamp: PREVIEW_LINES,
            }}
          >
            {previewOf(item.content)}
          </p>
        )}

        <div className="mt-[2px] flex h-[18px] items-center gap-[var(--gap-badge)] text-fs-xs text-faint">
          <time
            dateTime={new Date(item.created_at).toISOString()}
            title={absoluteTime(item.created_at)}
          >
            {shortAge(item.created_at)}
          </time>
          {item.pinned && (
            <span className="flex items-center gap-[2px] text-accent-2">
              <Pin size={10} aria-hidden="true" />
              Pinned
            </span>
          )}
          {item.is_sensitive && (
            <span className="text-c-secret">· Sensitive</span>
          )}
        </div>
      </div>

      <div
        className={cn(
          "flex shrink-0 items-center gap-[2px] opacity-0 transition-opacity duration-[var(--dur-fast)]",
          "group-hover:opacity-100 group-focus-within:opacity-100",
          active && "opacity-100",
        )}
      >
        {revealed && (
          <IconButton
            label="Hide sensitive content"
            size="sm"
            tabIndex={actionTab}
            onClick={(event) => {
              event.stopPropagation();
              onHide();
            }}
          >
            <EyeOff size={14} aria-hidden="true" />
          </IconButton>
        )}
        <IconButton
          label="Copy to clipboard"
          size="sm"
          tabIndex={actionTab}
          onClick={(event) => {
            event.stopPropagation();
            onCopy(item);
          }}
        >
          <Copy size={14} aria-hidden="true" />
        </IconButton>
        <IconButton
          label={item.pinned ? "Unpin item" : "Pin item"}
          size="sm"
          tabIndex={actionTab}
          onClick={(event) => {
            event.stopPropagation();
            onTogglePin(item);
          }}
        >
          {item.pinned ? (
            <PinOff size={14} aria-hidden="true" />
          ) : (
            <Pin size={14} aria-hidden="true" />
          )}
        </IconButton>
        <IconButton
          label="Delete item"
          tone="danger"
          size="sm"
          tabIndex={actionTab}
          onClick={(event) => {
            event.stopPropagation();
            onDelete(item);
          }}
        >
          <Trash2 size={14} aria-hidden="true" />
        </IconButton>
      </div>
    </div>
  );
}

/**
 * A plain `memo`, not the 20-field comparator manifest §9.1 calls a
 * maintenance hazard. Every prop is either a primitive or a stable callback,
 * so the default shallow compare is correct and stays correct when a field is
 * added to `Item`.
 */
export const HistoryRow = memo(HistoryRowImpl);
