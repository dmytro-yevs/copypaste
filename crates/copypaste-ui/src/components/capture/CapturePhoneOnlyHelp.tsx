import { useMutation } from "@tanstack/react-query";

import { useTranslation } from "@/i18n";
import {
  type CaptureSnapshot,
  captureOpenDeveloperOptions,
  captureOpenShizuku,
  captureRequestBatteryExemption,
} from "@/lib/ipc";
import { toFriendly } from "@/lib/errors";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";

interface CapturePhoneOnlyHelpProps {
  snapshot: CaptureSnapshot;
  disabled?: boolean;
}

export function CapturePhoneOnlyHelp({
  snapshot,
  disabled = false,
}: CapturePhoneOnlyHelpProps) {
  const { t } = useTranslation();
  const run = useMutation<void, unknown, () => Promise<void>>({
    mutationFn: (action) => action(),
    onError: (raw) => toast.error(toFriendly(raw)),
  });

  if (snapshot.rung === "desktop" || !snapshot.shizuku.supported) return null;

  const busy = disabled || run.isPending;

  return (
    <section className="flex flex-col gap-s-2 rounded-lg border border-border bg-card p-s-3">
      <h2 className="text-sm font-medium">{t("capture.help.title")}</h2>
      <p className="text-xs text-muted-foreground">{t("capture.help.body")}</p>
      <div className="flex flex-col gap-s-2">
        {snapshot.shizuku.installed ? (
          <Button
            className="w-full"
            variant="outline"
            disabled={busy}
            onClick={() => run.mutate(() => captureOpenShizuku())}
          >
            {t("capture.help.openShizuku")}
          </Button>
        ) : null}
        <Button
          className="w-full"
          variant="outline"
          disabled={busy}
          onClick={() => run.mutate(() => captureOpenDeveloperOptions())}
        >
          {t("capture.help.openDeveloperOptions")}
        </Button>
        <Button
          className="w-full"
          variant="outline"
          disabled={busy}
          onClick={() => run.mutate(() => captureRequestBatteryExemption())}
        >
          {t("capture.help.requestBattery")}
        </Button>
      </div>
    </section>
  );
}
