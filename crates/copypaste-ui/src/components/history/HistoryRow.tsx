/**
 * INV-10: a sensitive item's plaintext is *absent*, not blurred — the bridge
 * sends `content: null`, and the accessible name is a fixed string (AT-13).
 *
 * **Click selects, double-click copies.** A single click that overwrites the
 * system clipboard is a destructive default. The explicit Copy button is the
 * path a screen reader or a touch user takes, which is also why the actions are
 * always visible: Android has no hover, so a hover-revealed control does not
 * exist there.
 *
 * The body button and the action buttons are siblings, never nested: a control
 * inside a control is the `nested-interactive` violation INV-8 is about.
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

/** A11Y-3, verbatim from the manifest. */
export const SENSITIVE_A11Y_LABEL = "Sensitive item, hidden — activate to reveal";
export const SENSITIVE_REVEAL_LABEL =
  "Sensitive content hidden — activate to reveal";
const SENSITIVE_PLACEHOLDER = "Sensitive content hidden";
const EMPTY_LABEL = "Empty item";

/** No relative age: the live region mirrors this on every selection change,
 *  and an age that ticks would re-announce a selection that did not change. */
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

/** 36px under a mouse, 48px under a finger: `--sz-iconbtn` carries the
 *  coarse-pointer floor, so one token moves every target. */
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
  // Roving: 200 rows must not be 1000 tab stops.
  const tab = active ? 0 : -1;

  return (
    <div
      className={cn(
        "group relative flex h-full items-start gap-[var(--gap-row)] overflow-hidden rounded-lg px-[var(--pad-row-x)] py-[var(--pad-row-y)] transition-colors duration-[var(--dur-fast)]",
        "hover:bg-accent",
        (active || flashing) && "bg-selected",
        // Selection is carried by the edge, not the fill: --selected differs
        // from --bg by ~1.1:1, so a selected row and a hovered row are
        // indistinguishable without it (WCAG 1.4.11).
        active &&
          "before:absolute before:inset-y-[var(--sel-bar-inset)] before:left-0 before:w-[var(--sel-bar-w)] before:rounded-full before:bg-selected-edge before:content-['']",
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
          // The withheld slot stands in for content that was never delivered.
          // Not a blur: a blur says the content is present behind a filter,
          // which for a screenshot or a shoulder is both a lie and a leak.
          <span className="flex items-center gap-2 rounded-sm border border-withheld-border bg-withheld px-2 py-0.5 text-sm text-withheld-fg">
            <ShieldAlert size={12} aria-hidden="true" />
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

/** Plain `memo`, not v1's twenty-field comparator: that one silently stopped
 *  re-rendering when `Item` grew a field (§9.1). */
export const HistoryRow = memo(HistoryRowImpl);
