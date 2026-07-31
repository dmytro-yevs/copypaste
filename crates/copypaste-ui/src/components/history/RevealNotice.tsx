import { ShieldAlert } from "lucide-react";

import { useTranslation } from "@/i18n";

interface RevealNoticeProps {
  message: string | null;
  onDismiss: () => void;
}

/** A refused reveal is a state, not a failure — see `useReveal`. It is
 *  dismissible, and it never carries a raw error (INV-12). */
export function RevealNotice({ message, onDismiss }: RevealNoticeProps) {
  const { t } = useTranslation();
  if (!message) return null;

  return (
    <div
      role="alert"
      className="flex shrink-0 items-start gap-s-2 border-t border-warn/20 bg-warn/15 px-s-3 py-s-2 text-xs text-warn-strong"
    >
      <ShieldAlert size={14} aria-hidden="true" className="mt-px shrink-0" />
      <span className="min-w-0 flex-1">{message}</span>
      <button
        type="button"
        onClick={onDismiss}
        className="shrink-0 underline underline-offset-2 outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
      >
        {t("common.dismiss")}
      </button>
    </div>
  );
}
