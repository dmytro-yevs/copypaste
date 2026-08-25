import { useEffect, useState } from "react";

import { VisuallyHidden } from "@/components/ui";
import { t } from "@/i18n";

export function UndoCountdown({ ms }: { ms: number }) {
  const total = Math.ceil(ms / 1000);
  const [left, setLeft] = useState(total);

  useEffect(() => {
    const timer = window.setInterval(
      () => setLeft((seconds) => (seconds > 0 ? seconds - 1 : 0)),
      1000,
    );
    return () => window.clearInterval(timer);
  }, []);

  return (
    <span>
      <VisuallyHidden>
        {t("history.toast.undoWindow", { count: total })}
      </VisuallyHidden>
      <span aria-hidden="true">
        {t("history.toast.undoIn", { count: left })}
      </span>
    </span>
  );
}
