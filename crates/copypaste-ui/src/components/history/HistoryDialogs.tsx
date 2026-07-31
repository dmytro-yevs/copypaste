import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { buttonVariants } from "@/components/ui/button";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";

interface Confirmation {
  readonly open: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: () => void;
}

interface HistoryDialogsProps {
  reveal: Confirmation;
  bulkDelete: Confirmation & { count: number };
  clear: Confirmation;
}

/**
 * The three confirmations the History screen can raise. Each is driven by its
 * own piece of state, held by whichever controller owns the operation, so only
 * one can ever be open (INV-18).
 */
export function HistoryDialogs({
  reveal,
  bulkDelete,
  clear,
}: HistoryDialogsProps) {
  const { t } = useTranslation();

  return (
    <>
      <AlertDialog
        open={reveal.open}
        onOpenChange={(open) => !open && reveal.onCancel()}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("history.reveal.confirm.title")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.reveal.confirm.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction onClick={reveal.onConfirm}>
              {t("history.reveal.confirm.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Bulk delete has no undo window, unlike the single-row delete
          (§3.1.9), so this dialog is the only gate in front of it. */}
      <AlertDialog
        open={bulkDelete.open}
        onOpenChange={(open) => !open && bulkDelete.onCancel()}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("history.bulkDelete.title", { count: bulkDelete.count })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.bulkDelete.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={bulkDelete.onConfirm}
            >
              {t("history.bulkDelete.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={clear.open}
        onOpenChange={(open) => !open && clear.onCancel()}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("history.clear.title")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.clear.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={clear.onConfirm}
            >
              {t("history.clear.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
