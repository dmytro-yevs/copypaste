/**
 * The switch is bound to the probe, not to what the user just asked for: the
 * write goes through the Shizuku user service and can fail, and a switch that showed "off"
 * for a notice still appearing would be the same class of lie as reporting
 * capture that is not running.
 */
import { useState } from "react";

import { SettingsRow } from "@/components/shared";
import { Switch } from "@/components/ui";
import { ToastConsentDialog } from "./ToastConsentDialog";
import { useCaptureMutation } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { captureSetToastSuppressed } from "@/lib/ipc";

export function ToastNotice({ suppressed }: { suppressed: boolean }) {
  const { t } = useTranslation();
  const [asking, setAsking] = useState(false);
  const run = useCaptureMutation();

  return (
    <>
      <SettingsRow
        title={t("capture.toast.row.title")}
        description={t("capture.toast.row.body")}
      >
        <Switch
          checked={suppressed}
          disabled={run.isPending}
          aria-label={t("capture.toast.row.title")}
          onCheckedChange={(next) => {
            // Only the dialog may pass an acknowledgement. Restoring the
            // notice is never gated — a user who wants the warning back gets
            // it back, with nothing to read first.
            if (next) setAsking(true);
            else run.mutate(() => captureSetToastSuppressed(false, false));
          }}
        />
      </SettingsRow>

      <ToastConsentDialog open={asking} onOpenChange={setAsking} />
    </>
  );
}
