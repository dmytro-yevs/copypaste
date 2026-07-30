/**
 * Asking to turn off one of the OS's privacy indicators.
 *
 * The gate is in Rust (`authorise_toast`), so nothing here can bypass it; this
 * is how the user is asked, and the only thing it must never do is report an
 * acknowledgement that was not given. Two rules follow, and both are tested:
 *
 * * The explanation is **fetched**, never copied. It is the same string
 *   `authorise_toast` means by "explained", and a second copy in the catalogue
 *   would be a second thing to keep true.
 * * When the explanation is not on screen there is **no confirm button** —
 *   absent rather than disabled. There is nothing to consent to yet.
 */
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useCaptureMutation, useToastExplanation } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { captureSetToastSuppressed } from "@/lib/ipc";

interface ToastConsentDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ToastConsentDialog({
  open,
  onOpenChange,
}: ToastConsentDialogProps) {
  const { t } = useTranslation();
  const explanation = useToastExplanation(open);
  const run = useCaptureMutation();

  const shown = explanation.data;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("capture.toast.dialog.title")}</DialogTitle>
          <DialogDescription>
            {shown ??
              (explanation.isPending
                ? t("capture.toast.dialog.loading")
                : t("capture.toast.dialog.unavailable"))}
          </DialogDescription>
        </DialogHeader>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>

          {shown !== undefined && (
            <Button
              disabled={run.isPending}
              onClick={() =>
                run.mutate(() => captureSetToastSuppressed(true, true), {
                  onSuccess: () => onOpenChange(false),
                })
              }
            >
              {t("capture.toast.dialog.confirm")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
