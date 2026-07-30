/**
 * "3 items could not be read" (parity finding 17, `CopyPaste-00zz`).
 *
 * v1 returned a skipped count with every page so the UI could say this; v2
 * dropped it and the page just came back shorter, which is indistinguishable
 * from having copied less.
 *
 * **A state, not an error.** The rows that opened are fine and nothing the user
 * does will fix the ones that did not, so this is a quiet line rather than a
 * banner with a retry button. It is `aria-live="polite"` because it appears
 * without the user having asked for anything.
 *
 * There is deliberately no "delete the unreadable ones" action. Data loss is
 * the worst outcome (CLAUDE.md rule 4), and a row that will not decrypt today
 * may be a keychain problem that is fixable tomorrow — offering to destroy it
 * turns a recoverable state into a permanent one.
 */
import { FileWarning } from "lucide-react";

interface SkippedNoticeProps {
  count: number;
}

export function SkippedNotice({ count }: SkippedNoticeProps) {
  if (count <= 0) return null;

  return (
    <div
      aria-live="polite"
      className="flex shrink-0 items-center gap-s-2 border-b border-divider bg-raised-1 px-s-3 py-s-1 text-xs text-muted-foreground"
    >
      <FileWarning size={13} aria-hidden="true" className="shrink-0" />
      <span>
        {count} item{count === 1 ? "" : "s"} could not be read and{" "}
        {count === 1 ? "is" : "are"} not shown.
      </span>
    </div>
  );
}
