import { CloudOff, Copy, Eye, EyeOff, LoaderCircle, ShieldAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { wontSync } from "@/components/history/origin";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { MONO_KINDS, absoluteTime, kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import { kindLabel } from "@/lib/view";

interface HistoryDetailProps {
  /** `null` closes the view. Resolved from the id every render, so an item
   *  deleted underneath the reader closes it rather than showing a ghost. */
  item: Item | null;
  origin: string | null;
  revealedContent: string | null;
  revealPending: boolean;
  onReveal: (item: Item) => void;
  onHide: () => void;
  onCopy: (item: Item) => void;
  onClose: () => void;
  /** Where focus goes when the view closes. The trigger is a row inside a
   *  virtualised list and may not exist by then, and Radix's own restore would
   *  drop focus on `<body>`. */
  onReturnFocus: () => void;
}

export function HistoryDetail({
  item,
  origin,
  revealedContent,
  revealPending,
  onReveal,
  onHide,
  onCopy,
  onClose,
  onReturnFocus,
}: HistoryDetailProps) {
  const { t } = useTranslation();

  const revealed = item !== null && revealedContent !== null;
  const masked = item?.is_sensitive === true && !revealed;
  // The plaintext is read from the reveal, never held here: this component
  // keeps no copy of it, so INV-11's expiry re-masks the view by itself.
  const body = revealed ? revealedContent : (item?.content ?? null);
  const kind = item ? kindOf(item) : "text";

  const meta = item
    ? [absoluteTime(item.created_at), kindLabel(kindOf(item))]
    : [];
  if (item && origin !== null) {
    meta.push(`${t("history.row.fromPrefix")} ${origin}`);
  }

  return (
    <Dialog open={item !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        className="max-h-[85vh] gap-s-3"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          onReturnFocus();
        }}
      >
        <DialogHeader>
          <DialogTitle>{t("history.detail.title")}</DialogTitle>
          <DialogDescription>{meta.join(" · ")}</DialogDescription>
        </DialogHeader>

        {item && wontSync(item) && (
          <p className="flex items-start gap-s-1 text-xs text-warn-strong">
            <CloudOff size={12} aria-hidden="true" className="mt-px shrink-0" />
            {t("history.row.wontSync")}
          </p>
        )}

        {masked ? (
          <div className="flex flex-col items-start gap-s-2 rounded-md border border-withheld-border bg-withheld p-s-3 text-sm text-withheld-fg">
            <span className="flex items-center gap-2">
              <ShieldAlert size={14} aria-hidden="true" />
              {t("history.row.sensitivePlaceholder")}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => item && onReveal(item)}
            >
              {revealPending ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" />
              ) : (
                <Eye aria-hidden="true" />
              )}
              {t("history.detail.reveal")}
            </Button>
          </div>
        ) : (
          <div
            role="region"
            aria-label={t("history.detail.contents")}
            tabIndex={0}
            className="max-h-[46vh] min-h-[var(--sz-iconbtn)] overflow-y-auto rounded-md border border-border-strong bg-panel p-s-2 outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
          >
            <p
              className={cn(
                "text-sm leading-normal break-words whitespace-pre-wrap text-foreground",
                MONO_KINDS.has(kind) && "font-mono text-xs",
              )}
            >
              {body === null || body === "" ? t("history.detail.empty") : body}
            </p>
          </div>
        )}

        <DialogFooter>
          {revealed && (
            <Button variant="outline" onClick={onHide}>
              <EyeOff aria-hidden="true" />
              {t("history.detail.hide")}
            </Button>
          )}
          <Button
            onClick={() => {
              if (item) onCopy(item);
              onClose();
            }}
          >
            <Copy aria-hidden="true" />
            {t("history.detail.copy")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
