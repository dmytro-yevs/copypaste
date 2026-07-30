/**
 * INV-10: a sensitive item's plaintext is *absent*, not blurred — the bridge
 * sends `content: null`, and the accessible name is a fixed string (AT-13).
 *
 * **Click selects, double-click copies.** A single click that overwrites the
 * system clipboard is a destructive default. The actions are always visible
 * because Android has no hover, so a hover-revealed control does not exist
 * there.
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
import { Checkbox } from "@/components/ui/checkbox";
import { t, useTranslation } from "@/i18n";
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

/**
 * No relative age: the live region mirrors this on every selection change, and
 * an age that ticks would re-announce a selection that did not change.
 *
 * The clip is concatenated onto a catalogue prefix rather than interpolated
 * into a message — no template ever takes an item's content as a variable.
 */
export function rowLabel(item: Item): string {
  const body = item.is_sensitive
    ? t("history.row.sensitiveName")
    : item.content === null
      ? t("history.row.empty")
      : previewOf(item.content);
  return item.pinned ? `${t("history.row.pinnedPrefix")} ${body}` : body;
}

interface HistoryRowProps {
  item: Item;
  active: boolean;
  flashing: boolean;
  revealedContent: string | null;
  revealPending: boolean;
  previewLines: number;
  /** Selection mode is on, so the row shows a checkbox and a click toggles it
   *  instead of selecting the row (§3.1.5). */
  selecting: boolean;
  checked: boolean;
  onToggleChecked: (item: Item) => void;
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
  selecting,
  checked,
  onToggleChecked,
  onSelect,
  onCopy,
  onTogglePin,
  onDelete,
  onReveal,
  onHide,
}: HistoryRowProps) {
  const { t: tr } = useTranslation();
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
      {selecting ? (
        // Wrapped in a full tap target: `--sz-iconbtn` is 36px under a mouse
        // and 48px under a finger, so the checkbox itself stays 16px of ink
        // without the touch target shrinking to match.
        <span
          className={cn(
            "-my-[var(--pad-row-y)] flex shrink-0 items-center justify-center",
            ACTION,
          )}
        >
          <Checkbox
            checked={checked}
            tabIndex={tab}
            aria-label={`${tr("history.row.selectPrefix")} ${rowLabel(item)}`}
            onCheckedChange={() => onToggleChecked(item)}
          />
        </span>
      ) : (
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
      )}

      <button
        type="button"
        aria-label={rowLabel(item)}
        title={tr(
          selecting ? "history.row.selectingHint" : "history.row.hint",
        )}
        tabIndex={tab}
        onClick={() => (selecting ? onToggleChecked(item) : onSelect(item))}
        // No double-click-to-copy while selecting: a second click there means
        // "deselect", and copying on it would put an item on the clipboard the
        // user was in the middle of un-choosing.
        onDoubleClick={selecting ? undefined : () => onCopy(item)}
        className="flex min-w-0 flex-1 flex-col items-start rounded-sm text-left outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
      >
        {masked ? (
          // The withheld slot stands in for content that was never delivered.
          // Not a blur: a blur says the content is present behind a filter,
          // which for a screenshot or a shoulder is both a lie and a leak.
          <span className="flex items-center gap-2 rounded-sm border border-withheld-border bg-withheld px-2 py-0.5 text-sm text-withheld-fg">
            <ShieldAlert size={12} aria-hidden="true" />
            {tr("history.row.sensitivePlaceholder")}
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
              {tr("history.row.pinnedBadge")}
            </span>
          )}
          {item.is_sensitive && (
            <span className="text-c-secret">
              {tr("history.row.sensitiveBadge")}
            </span>
          )}
        </span>
      </button>

      {/* Not rendered in selection mode — not merely hidden with a class. The
          bulk bar duplicates these, a Delete button beside a checkbox is one
          misclick from destroying the wrong thing (§3.1.5), and a
          `display: none` button is still in the accessibility tree and still a
          tab stop. */}
      {!selecting && (
        <div className="flex shrink-0 items-center gap-0.5">
          {item.is_sensitive &&
            (revealed ? (
              <Button
                variant="ghost"
                aria-label={tr("history.row.hide")}
                title={tr("history.row.hide")}
                tabIndex={tab}
                className={ACTION}
                onClick={onHide}
              >
                <EyeOff aria-hidden="true" />
              </Button>
            ) : (
              <Button
                variant="ghost"
                aria-label={tr("history.row.sensitiveReveal")}
                title={tr("history.row.reveal")}
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
            aria-label={tr("history.row.copy")}
            title={tr("history.row.copy")}
            tabIndex={tab}
            className={ACTION}
            onClick={() => onCopy(item)}
          >
            <Copy aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            aria-label={tr(item.pinned ? "history.row.unpin" : "history.row.pin")}
            title={tr(item.pinned ? "history.row.unpin" : "history.row.pin")}
            tabIndex={tab}
            className={ACTION}
            onClick={() => onTogglePin(item)}
          >
            {item.pinned ? (
              <PinOff aria-hidden="true" />
            ) : (
              <Pin aria-hidden="true" />
            )}
          </Button>
          <Button
            variant="ghost"
            aria-label={tr("history.row.delete")}
            title={tr("history.row.delete")}
            tabIndex={tab}
            className={cn(ACTION, "hover:text-err-strong")}
            onClick={() => onDelete(item)}
          >
            <Trash2 aria-hidden="true" />
          </Button>
        </div>
      )}
    </div>
  );
}

/** Plain `memo`, not v1's twenty-field comparator: that one silently stopped
 *  re-rendering when `Item` grew a field (§9.1). */
export const HistoryRow = memo(HistoryRowImpl);
