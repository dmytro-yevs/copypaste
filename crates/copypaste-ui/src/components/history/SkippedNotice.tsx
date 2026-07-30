/**
 * "3 items could not be read" (parity finding 17, `CopyPaste-00zz`). Without it
 * a page that dropped undecryptable rows just comes back shorter, which is
 * indistinguishable from having copied less.
 *
 * **A state, not an error.** Nothing the user does will fix the rows that did
 * not open, so this is a quiet `aria-live="polite"` line rather than a banner
 * with a retry.
 *
 * There is deliberately no "delete the unreadable ones" action. Data loss is
 * the worst outcome (CLAUDE.md rule 4), and a row that will not decrypt today
 * may be a keychain problem that is fixable tomorrow.
 */
import { FileWarning } from "lucide-react";

import { useTranslation } from "@/i18n";

interface SkippedNoticeProps {
  count: number;
}

export function SkippedNotice({ count }: SkippedNoticeProps) {
  const { t } = useTranslation();
  if (count <= 0) return null;

  return (
    <div
      aria-live="polite"
      className="flex shrink-0 items-center gap-s-2 border-b border-divider bg-raised-1 px-s-3 py-s-1 text-xs text-muted-foreground"
    >
      <FileWarning size={13} aria-hidden="true" className="shrink-0" />
      <span>{t("history.skipped", { count })}</span>
    </div>
  );
}
