/**
 * One history row.
 *
 * INV-10: a sensitive item's plaintext is *absent*, not blurred — the bridge
 * sends `content: null`. Nothing here can reconstruct it, and the row's
 * accessible name is a fixed string (AT-13).
 *
 * INV-8: `role="listitem"`, never `role="option"` — option is
 * `childrenPresentational` and would flatten these buttons into an axe
 * `nested-interactive` violation. For the same reason the body button and the
 * action buttons are siblings; nesting them would recreate the violation
 * under a different tag.
 *
 * **Click selects, double-click copies.** Selecting and copying are different
 * intents, and a single click that overwrites the system clipboard is a
 * destructive default. Keyboard: Enter on the focused list copies (§3.1.4);
 * every row also carries an explicit Copy button, which is the path a screen
 * reader or a touch user takes.
 *
 * Actions are always visible. Hover is not available on Android, so a
 * hover-revealed control is a control that does not exist there.
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

/** A11Y-3, exact strings from the manifest. */
export const SENSITIVE_A11Y_LABEL = "Sensitive item, hidden — activate to reveal";
export const SENSITIVE_REVEAL_LABEL =
  "Sensitive content hidden — activate to reveal";
const SENSITIVE_PLACEHOLDER = "Sensitive content hidden";
const EMPTY_LABEL = "Empty item";

/**
 * Carries no relative age: the age changes on its own, and the live region
 * mirrors this string on every selection change (INV-9).
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
  /** Present only while this row is the revealed one. */
  revealedContent: string | null;
  revealPending: boolean;
  previewLines: number;
  onSelect: (item: Item) => void;
  onCopy: (item: Item) => void;
  onTogglePin: (item: Item) => void;
  onDelete: (item: Item) => void;
  onReveal: (item: Item) => void;
  onHide: () => void;
}

/** 36px under a mouse, 48px under a finger — `--sz-iconbtn` carries the
 *  coarse-pointer floor so one token moves every target. */
const ACTION = "size-[var(--sz-iconbtn)]";

function HistoryRowImpl({
  item,
  active,
  flashing,
  revealedContent,
  revealPending,
  previewLines,
  onSelect,
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
  // Roving tabindex: 200 rows must not be 1000 tab stops. Arrow keys on the
  // list move the selection, which moves what is tabbable.
  const tab = active ? 0 : -1;

  return (
    <div
      className={cn(
        "group relative flex h-full items-start gap-[var(--gap-row)] overflow-hidden rounded-lg border border-transparent px-[var(--pad-row-x)] py-[var(--pad-row-y)] transition-colors duration-[var(--dur-fast)]",
        "hover:bg-accent",
        active && "border-border bg-selected",
        flashing && "bg-selected",
      )}
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

      <button
        type="button"
        aria-label={rowLabel(item)}
        title="Click to select · double-click to copy"
        tabIndex={tab}
        onClick={() => onSelect(item)}
        onDoubleClick={() => onCopy(item)}
        className="flex min-w-0 flex-1 flex-col items-start rounded-sm text-left outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
      >
        {masked ? (
          <span className="flex items-center gap-2 text-sm text-c-secret">
            <span
              className="h-[var(--fs-sm)] w-20 rounded-xs bg-current opacity-25"
              aria-hidden="true"
            />
            {SENSITIVE_PLACEHOLDER}
          </span>
        ) : (
          <span
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
          </span>
        )}

        <span className="mt-1 flex h-[18px] items-center gap-2 text-xs text-muted-foreground">
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
          {item.is_sensitive && <span className="text-c-secret">· Sensitive</span>}
        </span>
      </button>

      <div className="flex shrink-0 items-center gap-0.5">
        {item.is_sensitive &&
          (revealed ? (
            <Button
              variant="ghost"
              aria-label="Hide sensitive content"
              title="Hide sensitive content"
              tabIndex={tab}
              className={ACTION}
              onClick={onHide}
            >
              <EyeOff aria-hidden="true" />
            </Button>
          ) : (
            <Button
              variant="ghost"
              aria-label={SENSITIVE_REVEAL_LABEL}
              title="Reveal sensitive content"
              tabIndex={tab}
              className={ACTION}
              onClick={() => onReveal(item)}
            >
              {revealPending ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" />
              ) : (
                <Eye aria-hidden="true" />
              )}
            </Button>
          ))}
        <Button
          variant="ghost"
          aria-label="Copy to clipboard"
          title="Copy to clipboard"
          tabIndex={tab}
          className={ACTION}
          onClick={() => onCopy(item)}
        >
          <Copy aria-hidden="true" />
        </Button>
        <Button
          variant="ghost"
          aria-label={item.pinned ? "Unpin item" : "Pin item"}
          title={item.pinned ? "Unpin item" : "Pin item"}
          tabIndex={tab}
          className={ACTION}
          onClick={() => onTogglePin(item)}
        >
          {item.pinned ? <PinOff aria-hidden="true" /> : <Pin aria-hidden="true" />}
        </Button>
        <Button
          variant="ghost"
          aria-label="Delete item"
          title="Delete item"
          tabIndex={tab}
          className={cn(ACTION, "hover:text-err-strong")}
          onClick={() => onDelete(item)}
        >
          <Trash2 aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}

/**
 * Plain `memo`, not v1's twenty-field comparator: every prop is a primitive or
 * a stable callback, so the shallow compare stays correct when `Item` grows a
 * field. The comparator silently stopped re-rendering when it did not (§9.1).
 */
export const HistoryRow = memo(HistoryRowImpl);
