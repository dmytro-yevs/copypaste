import { useState } from "react";
import {
  Copy,
  Expand,
  MoreHorizontal,
  Pin,
  PinOff,
  Trash2,
  X,
} from "lucide-react";

import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import type { Item } from "@/lib/ipc";

const ACTION = "size-[var(--sz-iconbtn)]";

interface HistoryRowActionsProps {
  item: Item;
  android: boolean;
  active: boolean;
  onCopy: (item: Item) => void;
  onTogglePin: (item: Item) => void;
  onDelete: (item: Item) => void;
  onOpen: (item: Item) => void;
}

export function HistoryRowActions({
  item,
  android,
  active,
  onCopy,
  onTogglePin,
  onDelete,
  onOpen,
}: HistoryRowActionsProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const tabIndex = active ? 0 : -1;

  function run(action: () => void) {
    setOpen(false);
    window.setTimeout(action, 0);
  }

  if (android) {
    return (
      <Dialog open={open} onOpenChange={setOpen}>
        <Button
          variant="ghost"
          size="icon"
          aria-label={t("history.row.actions")}
          title={t("history.row.actions")}
          tabIndex={tabIndex}
          onClick={() => setOpen(true)}
        >
          <MoreHorizontal aria-hidden="true" />
        </Button>

        <DialogContent
          showCloseButton={false}
          className="top-auto right-[calc(var(--inset-right)+var(--s-2))] bottom-[calc(var(--inset-bottom)+var(--s-2))] left-[calc(var(--inset-left)+var(--s-2))] w-auto max-h-[calc(100dvh-var(--inset-top)-var(--inset-bottom)-var(--s-4))] max-w-none translate-x-0 translate-y-0 rounded-xl p-4"
        >
          <DialogHeader>
            <DialogTitle>{t("history.row.actions")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-1">
            <Button
              variant="ghost"
              className="justify-start"
              onClick={() => run(() => onCopy(item))}
            >
              <Copy aria-hidden="true" />
              {t("history.row.copy")}
            </Button>
            <Button
              variant="ghost"
              className="justify-start"
              onClick={() => run(() => onOpen(item))}
            >
              <Expand aria-hidden="true" />
              {t("history.row.open")}
            </Button>
            <Button
              variant="ghost"
              className="justify-start"
              onClick={() => run(() => onTogglePin(item))}
            >
              {item.pinned ? <PinOff aria-hidden="true" /> : <Pin aria-hidden="true" />}
              {t(item.pinned ? "history.row.unpin" : "history.row.pin")}
            </Button>
            <Button
              variant="ghost"
              className="justify-start text-err-strong hover:text-err-strong"
              onClick={() => run(() => onDelete(item))}
            >
              <Trash2 aria-hidden="true" />
              {t("history.row.delete")}
            </Button>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">
                <X aria-hidden="true" />
                {t("common.cancel")}
              </Button>
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }

  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <Button
        variant="ghost"
        aria-label={t("history.row.open")}
        title={t("history.row.open")}
        tabIndex={tabIndex}
        className={ACTION}
        onClick={() => onOpen(item)}
      >
        <Expand aria-hidden="true" />
      </Button>
      <Button
        variant="ghost"
        aria-label={t("history.row.copy")}
        title={t("history.row.copy")}
        tabIndex={tabIndex}
        className={ACTION}
        onClick={() => onCopy(item)}
      >
        <Copy aria-hidden="true" />
      </Button>
      <Button
        variant="ghost"
        aria-label={t(item.pinned ? "history.row.unpin" : "history.row.pin")}
        title={t(item.pinned ? "history.row.unpin" : "history.row.pin")}
        tabIndex={tabIndex}
        className={ACTION}
        onClick={() => onTogglePin(item)}
      >
        {item.pinned ? <PinOff aria-hidden="true" /> : <Pin aria-hidden="true" />}
      </Button>
      <Button
        variant="ghost"
        aria-label={t("history.row.delete")}
        title={t("history.row.delete")}
        tabIndex={tabIndex}
        className={cn(ACTION, "hover:text-err-strong")}
        onClick={() => onDelete(item)}
      >
        <Trash2 aria-hidden="true" />
      </Button>
    </div>
  );
}
