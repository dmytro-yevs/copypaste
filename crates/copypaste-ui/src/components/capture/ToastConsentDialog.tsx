/**
 * Turning off one of the OS's privacy indicators. The gate is in Rust
 * (`authorise_toast`); this is how the user is asked, and the one thing it must
 * never do is report an acknowledgement that was not given. Both rules below
 * are tested:
 *
 * * The explanation is fetched, never copied — a second copy in the catalogue
 *   is a second thing to keep true.
 * * With the explanation off screen there is no confirm button, absent rather
 *   than disabled: there is nothing to consent to yet.
 */
import { Check, X } from "lucide-react";

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
            <X aria-hidden="true" />
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
              <Check aria-hidden="true" />
              {t("capture.toast.dialog.confirm")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
