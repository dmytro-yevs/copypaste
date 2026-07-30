/**
 * One history row.
 *
 * Two rules here are the ones v1 paid for:
 *
 *  - **A sensitive item never renders its content** (INV-10, CLAUDE.md rule 4).
 *    It is not blurred, it is *absent*: the bridge sends `content: null`, so
 *    there is nothing in this component to leak into a screenshot or the
 *    accessibility tree. The row's accessible name is a fixed string with no
 *    substring of the item in it (AT-13). Revealing is an explicit act that
 *    fetches the plaintext, and it re-hides on blur and after 10s (INV-11).
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
import {
  Copy,
  Eye,
  EyeOff,
  LoaderCircle,
  Pin,
  PinOff,
  ShieldAlert,
  Trash2,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/cn";
import {
  KIND_TEXT_CLASS,
  MONO_KINDS,
  absoluteTime,
  kindOf,
  previewOf,
  shortAge,
} from "@/lib/format";
import type { Item } from "@/lib/ipc";

/** INV-10 / A11Y-3 — exact strings from manifest 06. */
export const SENSITIVE_A11Y_LABEL = "Sensitive item, hidden — activate to reveal";
export const SENSITIVE_REVEAL_LABEL =
  "Sensitive content hidden — activate to reveal";
const SENSITIVE_PLACEHOLDER = "Sensitive content hidden";
const EMPTY_LABEL = "Empty item";

/**
 * The row's accessible name, and the string the live region mirrors.
 *
 * Deliberately free of the relative age: the age changes on its own, and a live
 * region that re-announces every minute is worse than one that announces only
 * on selection changes (INV-9).
 */
export function rowLabel(item: Item): string {
  const body = item.is_sensitive
    ? SENSITIVE_A11Y_LABEL
    : item.content === null
      ? EMPTY_LABEL
      : previewOf(item.content);
  return item.pinned ? `Pinned. ${body}` : body;
}

interface HistoryRowProps {
  item: Item;
  active: boolean;
  flashing: boolean;
  /** The plaintext, present only while this row is the revealed one. */
  revealedContent: string | null;
  revealPending: boolean;
  previewLines: number;
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
  flashing,
  revealedContent,
  revealPending,
  previewLines,
  onActivate,
  onCopy,
  onTogglePin,
  onDelete,
  onReveal,
  onHide,
}: HistoryRowProps) {
  const kind = kindOf(item);
  const revealed = revealedContent !== null;
  const masked = item.is_sensitive && !revealed;
  const body = revealed ? revealedContent : item.content;
  // Action buttons are only in the tab order for the active row — a roving
  // tabindex, so 200 rows do not mean 800 tab stops.
  const actionTab = active ? 0 : -1;

  return (
    <div
      className={cn(
        "group relative flex h-full items-start gap-[var(--gap-row)] overflow-hidden rounded-lg border border-transparent px-[var(--pad-row-x)] py-[var(--pad-row-y)] transition-colors duration-[var(--dur-fast)]",
        "hover:bg-accent",
        active && "border-border bg-selected",
        flashing && "bg-selected",
      )}
      onClick={() => onActivate(item)}
    >
      <span
        aria-hidden="true"
        className={cn(
          "mt-px flex size-[var(--icon-lg)] shrink-0 items-center justify-center",
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
            className="flex w-fit items-center gap-2 rounded-sm text-sm text-c-secret outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
          >
            <span
              className="h-[var(--fs-sm)] w-20 rounded-xs bg-current opacity-25"
              aria-hidden="true"
            />
            <span>{SENSITIVE_PLACEHOLDER}</span>
            {revealPending ? (
              <LoaderCircle size={12} aria-hidden="true" className="animate-spin" />
            ) : (
              <Eye size={12} aria-hidden="true" />
            )}
          </button>
        ) : (
          <p
            className={cn(
              "min-w-0 overflow-hidden text-sm leading-normal break-words whitespace-pre-wrap text-foreground",
              MONO_KINDS.has(kind) && "font-mono text-xs",
            )}
            style={{
              display: "-webkit-box",
              WebkitBoxOrient: "vertical",
              WebkitLineClamp: previewLines,
            }}
          >
            {body === null ? "" : previewOf(body)}
          </p>
        )}

        <div className="mt-1 flex h-[18px] items-center gap-2 text-xs text-muted-foreground">
          <time
            dateTime={new Date(item.created_at).toISOString()}
            title={absoluteTime(item.created_at)}
          >
            {shortAge(item.created_at)}
          </time>
          {item.pinned && (
            <span className="flex items-center gap-0.5 text-brand-2">
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
          "flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity duration-[var(--dur-fast)]",
          "group-hover:opacity-100 group-focus-within:opacity-100",
          active && "opacity-100",
        )}
      >
        {revealed && (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Hide sensitive content"
            title="Hide sensitive content"
            tabIndex={actionTab}
            onClick={(event) => {
              event.stopPropagation();
              onHide();
            }}
          >
            <EyeOff aria-hidden="true" />
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Copy to clipboard"
          title="Copy to clipboard"
          tabIndex={actionTab}
          onClick={(event) => {
            event.stopPropagation();
            onCopy(item);
          }}
        >
          <Copy aria-hidden="true" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={item.pinned ? "Unpin item" : "Pin item"}
          title={item.pinned ? "Unpin item" : "Pin item"}
          tabIndex={actionTab}
          onClick={(event) => {
            event.stopPropagation();
            onTogglePin(item);
          }}
        >
          {item.pinned ? (
            <PinOff aria-hidden="true" />
          ) : (
            <Pin aria-hidden="true" />
          )}
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Delete item"
          title="Delete item"
          tabIndex={actionTab}
          className="hover:text-err-strong"
          onClick={(event) => {
            event.stopPropagation();
            onDelete(item);
          }}
        >
          <Trash2 aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}

/**
 * A plain `memo`, not the 20-field comparator manifest §9.1 calls a maintenance
 * hazard. Every prop is either a primitive or a stable callback, so the default
 * shallow compare is correct and stays correct when a field is added to `Item`.
 */
export const HistoryRow = memo(HistoryRowImpl);
