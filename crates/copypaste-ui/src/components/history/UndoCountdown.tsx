import { useEffect, useState } from "react";

import { useTranslation } from "react-i18next";

/**
 * Seconds left in an undo window.
 *
 * The ticking number is `aria-hidden` and the window is stated once in a
 * visually hidden sibling: a live region counting down interrupts a screen
 * reader five times for one action, and the length of the window is the part
 * that is actually worth hearing.
 */
export function UndoCountdown({ ms }: { ms: number }) {
  const { t } = useTranslation();
  const total = Math.ceil(ms / 1000);
  const [left, setLeft] = useState(total);

  useEffect(() => {
    const timer = setInterval(
      () => setLeft((seconds) => (seconds > 0 ? seconds - 1 : 0)),
      1000,
    );
    return () => clearInterval(timer);
  }, []);

  return (
    <>
      <span className="sr-only">
        {t("history.toast.undoWindow", { count: total })}
      </span>
      <span aria-hidden="true" data-testid="undo-countdown">
        {t("history.toast.undoIn", { count: left })}
      </span>
    </>
  );
}
