/**
 * Advertises ⌘1–⌘9, otherwise undiscoverable, and states permanently that
 * choosing an item does not paste it — the app synthesises no keystroke,
 * because that needs an Accessibility grant an ad-hoc-signed app loses on every
 * update (ADR-0001). Without the line the app looks broken: ⌘3, the window
 * vanishes, nothing appears.
 *
 * `pointer: coarse` hides it. There is no ⌘ on a phone.
 */
import { Trans } from "@/i18n";

interface QuickHintProps {
  /** ⌘1–⌘9 is inactive while a search is running (§3.5.3), so the hint that
   *  advertises it goes with it rather than lying. */
  searching: boolean;
}

const KBD = { kbd: <kbd className="font-sans" /> };

export function QuickHint({ searching }: QuickHintProps) {
  return (
    <p className="hidden shrink-0 items-center justify-center gap-s-2 border-t border-divider bg-panel px-s-3 py-s-1 text-[11px] text-muted-foreground [@media(pointer:fine)]:flex">
      <span>
        <Trans i18nKey="history.hint.move" components={KBD} />
      </span>
      <span aria-hidden="true">·</span>
      <span>
        <Trans i18nKey="history.hint.copy" components={KBD} />
      </span>
      {!searching && (
        <>
          <span aria-hidden="true">·</span>
          <span>
            <Trans i18nKey="history.hint.quickCopy" components={KBD} />
          </span>
        </>
      )}
      <span aria-hidden="true">·</span>
      <span className="text-foreground">
        <Trans i18nKey="history.hint.paste" components={KBD} />
      </span>
    </p>
  );
}
