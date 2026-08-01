/**
 * INV-10: a sensitive item's plaintext is *absent*, not blurred, and the
 * accessible name is a fixed string (AT-13).
 *
 * Click selects, double-click copies: a single click that overwrites the system
 * clipboard is a destructive default. Actions stay visible because Android has
 * no hover.
 *
 * The body button and the action buttons are siblings, never nested — nesting
 * is the `nested-interactive` violation INV-8 is about.
 */
import { memo } from "react";
import {
  CloudOff,
  Copy,
  MonitorSmartphone,
  Pin,
  ShieldAlert,
} from "lucide-react";

import { Checkbox } from "@/components/ui/checkbox";
import { HistoryRowActions } from "@/components/history/HistoryRowActions";
import { wontSync } from "@/components/history/origin";
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
import { isAndroidPlatform } from "@/lib/platform";

const CHECKBOX_ACTION = "size-[var(--sz-iconbtn)]";

/**
 * No relative age: the live region mirrors this on every selection change, and
 * an age that ticks would re-announce a selection that did not change.
 *
 * The clip is concatenated onto a catalogue prefix, never interpolated into a
 * message — no template takes an item's content as a variable.
 */
export function rowLabel(item: Item, origin: string | null = null): string {
  const body = item.is_sensitive
    ? t("history.row.sensitiveName")
    : item.content === null
      ? t("history.row.empty")
      : previewOf(item.content);
  const named = item.pinned ? `${t("history.row.pinnedPrefix")} ${body}` : body;

  const marks: string[] = [];
  if (origin !== null) marks.push(`${t("history.row.fromPrefix")} ${origin}`);
  if (wontSync(item)) marks.push(t("history.row.wontSync"));
  return marks.length === 0 ? named : `${named} · ${marks.join(" · ")}`;
}

interface HistoryRowProps {
  item: Item;
  active: boolean;
  flashing: boolean;
  revealedContent: string | null;
  revealPending: boolean;
  previewLines: number;
  /** A string rather than a record, so the memo holds. `null` when the clipping
   *  earns no marker — see `markedOrigins`. */
  origin: string | null;
  selecting: boolean;
  checked: boolean;
  onToggleChecked: (item: Item) => void;
  onSelect: (item: Item) => void;
  onCopy: (item: Item) => void;
  onTogglePin: (item: Item) => void;
  onDelete: (item: Item) => void;
  onReveal: (item: Item) => void;
  onHide: () => void;
  onOpen: (item: Item) => void;
}

function HistoryRowImpl({
  item,
  active,
  flashing,
  revealedContent,
  revealPending,
  previewLines,
  origin,
  selecting,
  checked,
  onToggleChecked,
  onSelect,
  onCopy,
  onTogglePin,
  onDelete,
  onReveal,
  onHide,
  onOpen,
}: HistoryRowProps) {
  const { t: tr } = useTranslation();
  const kind = kindOf(item);
  const revealed = revealedContent !== null;
  const masked = item.is_sensitive && !revealed;
  const body = revealed ? revealedContent : item.content;
  const stranded = wontSync(item);
  const android = isAndroidPlatform();
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
        // Wrapped in a full tap target so the checkbox stays 16px of ink
        // without the touch target shrinking to match.
        <span
          className={cn(
            "-my-[var(--pad-row-y)] flex shrink-0 items-center justify-center",
            CHECKBOX_ACTION,
          )}
        >
          <Checkbox
            checked={checked}
            tabIndex={tab}
            aria-label={`${tr("history.row.selectPrefix")} ${rowLabel(item, origin)}`}
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
        aria-label={rowLabel(item, origin)}
        title={tr(
          selecting ? "history.row.selectingHint" : "history.row.hint",
        )}
        tabIndex={tab}
        onClick={() => {
          if (selecting) onToggleChecked(item);
          else onSelect(item);
        }}
        // No double-click-to-copy while selecting: a second click there means
        // "deselect", and copying on it would put an item on the clipboard the
        // user was in the middle of un-choosing.
        onDoubleClick={selecting || android ? undefined : () => onCopy(item)}
        className="flex min-w-0 flex-1 flex-col items-start rounded-sm text-left outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
      >
        {masked ? (
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
          {origin !== null && (
            <span className="flex min-w-0 items-center gap-0.5">
              <MonitorSmartphone size={10} aria-hidden="true" className="shrink-0" />
              <span className="truncate">{origin}</span>
            </span>
          )}
          {/* Rendered here rather than beside the actions so that it survives
              selection mode, where the actions are not rendered at all
              (CopyPaste-f72f). */}
          {stranded && (
            <span
              className="flex shrink-0 items-center gap-0.5 text-warn-strong"
              title={tr("history.row.wontSync")}
            >
              <CloudOff size={10} aria-hidden="true" />
              {tr("history.row.wontSyncBadge")}
            </span>
          )}
        </span>
      </button>

      {/* Not rendered in selection mode, not merely hidden with a class: a
          `display: none` button is still in the accessibility tree and still a
          tab stop, and a Delete beside a checkbox is one misclick from
          destroying the wrong thing (§3.1.5). */}
      {!selecting && (
        <HistoryRowActions
          item={item}
          android={android}
          active={active}
          revealed={revealed}
          revealPending={revealPending}
          onCopy={onCopy}
          onTogglePin={onTogglePin}
          onDelete={onDelete}
          onReveal={onReveal}
          onHide={onHide}
          onOpen={onOpen}
        />
      )}
    </div>
  );
}

/** Plain `memo`, not v1's twenty-field comparator: that one silently stopped
 *  re-rendering when `Item` grew a field (§9.1). */
export const HistoryRow = memo(HistoryRowImpl);
