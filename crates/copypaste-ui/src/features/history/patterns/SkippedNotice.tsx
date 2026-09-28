/**
 * Parity finding 17 (`CopyPaste-00zz`): without it a page that dropped
 * undecryptable rows comes back shorter, which is indistinguishable from having
 * copied less.
 *
 * A state, not an error — nothing the user does will fix those rows — so a
 * quiet `aria-live="polite"` line rather than a banner with a retry.
 *
 * No "delete the unreadable ones" action: data loss is the worst outcome
 * (AGENTS.md rule 4), and a row that will not decrypt today may be a keychain
 * problem that is fixable tomorrow.
 */
import { StateView } from "@/components/shared/StateView";
import { useTranslation } from "@/i18n";

interface SkippedNoticeProps {
    count: number;
}

export function SkippedNotice({ count }: SkippedNoticeProps) {
    const { t } = useTranslation();
    if (count <= 0) return null;

    return (
        <StateView mode="info" placement="inline" role="status" aria-live="polite" icon="fileWarning" title={t("history.skipped", { count })} />
    );
}
