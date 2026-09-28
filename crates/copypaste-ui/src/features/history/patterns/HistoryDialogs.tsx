import { AlertDialog } from "@/components/ui";
import { useTranslation } from "@/i18n";

interface Confirmation {
  readonly open: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: () => void;
}

type HistoryDialogsProps = {
  reveal: Confirmation;
  bulkDelete: Confirmation & { count: number };
};

/**
 * The three confirmations the History screen can raise. Each is driven by its
 * own piece of state, held by whichever controller owns the operation, so only
 * one can ever be open (INV-18).
 */
export function HistoryDialogs({
  reveal,
  bulkDelete,
}: HistoryDialogsProps) {
  const { t } = useTranslation();

  return (
    <>
      <AlertDialog
        open={reveal.open}
        onOpenChange={(open) => !open && reveal.onCancel()}
        title={t("history.reveal.confirm.title")}
        description={t("history.reveal.confirm.body")}
        cancel={{ label: t("common.cancel") }}
        action={{ label: t("history.reveal.confirm.action"), onClick: reveal.onConfirm }}
      />

      {/* Bulk delete has no undo window, unlike the single-row delete
          (§3.1.9), so this dialog is the only gate in front of it. */}
      <AlertDialog
        open={bulkDelete.open}
        onOpenChange={(open) => !open && bulkDelete.onCancel()}
        title={t("history.bulkDelete.title", { count: bulkDelete.count })}
        description={t("history.bulkDelete.body")}
        cancel={{ label: t("common.cancel") }}
        action={{ label: t("history.bulkDelete.action"), onClick: bulkDelete.onConfirm, variant: "danger" }}
      />
    </>
  );
}
