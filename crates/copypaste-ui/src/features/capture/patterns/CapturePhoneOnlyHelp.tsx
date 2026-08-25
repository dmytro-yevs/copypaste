import { useMutation } from "@tanstack/react-query";
import { Icon } from "@/components/ui/icon";
import { toast } from "sonner";

import { Button, Surface } from "@/components/ui";
import { useTranslation } from "@/i18n";
import {
  type CaptureSnapshot,
  captureOpenDeveloperOptions,
  captureOpenShizuku,
  captureRequestBatteryExemption,
} from "@/lib/ipc";
import { toFriendly } from "@/lib/errors";
import styles from "./CapturePhoneOnlyHelp.module.css";

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
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <section className={styles.root}>
      <h2 className={styles.heading}>{t("capture.help.title")}</h2>
      <p className={styles.body}>{t("capture.help.body")}</p>
      <div className={styles.actions}>
        {snapshot.shizuku.installed ? (
          <Button
            className={styles.action}
            variant="secondary"
            disabled={busy}
            onClick={() => run.mutate(() => captureOpenShizuku())}
          >
            <Icon name="code" size="md" />
            {t("capture.help.openShizuku")}
          </Button>
        ) : null}
        <Button
          className={styles.action}
          variant="secondary"
          disabled={busy}
          onClick={() => run.mutate(() => captureOpenDeveloperOptions())}
        >
          <Icon name="code" size="md" />
          {t("capture.help.openDeveloperOptions")}
        </Button>
        <Button
          className={styles.action}
          variant="secondary"
          disabled={busy}
          onClick={() => run.mutate(() => captureRequestBatteryExemption())}
        >
          <Icon name="battery" size="md" />
          {t("capture.help.requestBattery")}
        </Button>
      </div>
      </section>
    </Surface>
  );
}
