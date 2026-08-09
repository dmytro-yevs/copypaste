import { useState } from "react";
import {
  CloudOff,
  Copy,
  Eye,
  EyeOff,
  ImageIcon,
  LoaderCircle,
  ShieldAlert,
} from "lucide-react";

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
import { HistoryImagePreview } from "@/components/history/HistoryImagePreview";
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
  fullContent: string | null;
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
  fullContent,
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
  const potentialFinding =
    item !== null && !item.is_sensitive ? item.sensitive_finding : null;
  const [shownFinding, setShownFinding] =
    useState<Item["sensitive_finding"]>(null);
  const potentialRevealed =
    potentialFinding !== null && shownFinding === potentialFinding;
  // Whole-item sensitive plaintext comes only from the reveal. This component
  // keeps no copy, so INV-11's expiry re-masks the view by itself.
  const body =
    potentialFinding !== null && !potentialRevealed
      ? potentialFinding.redacted_preview
      : revealed
        ? revealedContent
        : (fullContent ?? item?.content ?? null);
  const kind = item ? kindOf(item) : "text";
  const isImage = kind === "image";

  const meta = item
    ? [absoluteTime(item.created_at), kindLabel(kindOf(item))]
    : [];
  if (item && origin !== null) {
    meta.push(`${t("history.row.fromPrefix")} ${origin}`);
  }

  const close = () => {
    setShownFinding(null);
    onClose();
  };

  return (
    <Dialog open={item !== null} onOpenChange={(open) => !open && close()}>
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

        {potentialFinding !== null && (
          <p
            role="status"
            aria-label={t("history.row.potentialSensitiveWarning")}
            className="flex items-start gap-s-1 text-xs text-warn-strong"
          >
            <ShieldAlert size={12} aria-hidden="true" className="mt-px shrink-0" />
            {t("history.row.potentialSensitiveWarning")}
          </p>
        )}

        {masked ? (
          <button
            type="button"
            aria-label={t("history.row.sensitiveReveal")}
            aria-busy={revealPending || undefined}
            className="flex min-h-24 w-full flex-col justify-center gap-s-2 rounded-md border border-withheld-border bg-withheld p-s-3 text-left text-sm text-withheld-fg outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
            onClick={() => item && !revealPending && onReveal(item)}
          >
            <span aria-hidden="true" className="flex w-full flex-col gap-2">
              <span className="h-3 w-10/12 rounded-full bg-withheld-fg/25" />
              <span className="h-3 w-7/12 rounded-full bg-withheld-fg/25" />
              <span className="h-3 w-9/12 rounded-full bg-withheld-fg/25" />
            </span>
            {revealPending && (
              <LoaderCircle aria-hidden="true" className="animate-spin self-center" />
            )}
          </button>
        ) : isImage && item ? (
          <div
            role="region"
            aria-label={t("history.detail.image")}
            tabIndex={0}
            className="flex max-h-[54vh] min-h-48 w-full items-center justify-center overflow-hidden rounded-md border border-border-strong bg-panel p-s-3 outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
          >
            <HistoryImagePreview
              id={item.id}
              className="max-h-[calc(54vh-var(--s-6))] max-w-full"
              style={{ maxHeight: "calc(54vh - var(--s-6))" }}
              loadingLabel={t("history.detail.imageLoading")}
              failureLabel={t("history.detail.imageUnavailable")}
              title={t("history.detail.image")}
            />
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
          {potentialFinding !== null && (
            <Button
              variant="outline"
              aria-pressed={potentialRevealed}
              onClick={() =>
                setShownFinding(potentialRevealed ? null : potentialFinding)
              }
            >
              {potentialRevealed ? (
                <EyeOff aria-hidden="true" />
              ) : (
                <Eye aria-hidden="true" />
              )}
              {t(
                potentialRevealed
                  ? "history.row.hideOriginal"
                  : "history.row.showOriginal",
              )}
            </Button>
          )}
          {revealed && (
            <Button variant="outline" onClick={onHide}>
              <EyeOff aria-hidden="true" />
              {t("history.detail.hide")}
            </Button>
          )}
          <Button
            onClick={() => {
              if (item) onCopy(item);
              close();
            }}
          >
            {isImage ? <ImageIcon aria-hidden="true" /> : <Copy aria-hidden="true" />}
            {isImage ? t("history.detail.copyImage") : t("history.detail.copy")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
