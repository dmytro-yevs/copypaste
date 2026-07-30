/**
 * The footer hint strip.
 *
 * Two jobs, and the second is the important one.
 *
 * It advertises ⌘1–⌘9, which is otherwise undiscoverable: nothing on screen
 * suggests the rows are numbered.
 *
 * And it states, permanently, that **choosing an item does not paste it** —
 * the user presses ⌘V. That is not a limitation to hide. The app synthesises
 * no keystroke because doing so needs an Accessibility grant, and an
 * ad-hoc-signed app loses that grant on every update (ADR-0001, consequence 1).
 * Without this line the app looks broken to anyone who expects the paste to
 * happen: they press ⌘3, the window vanishes, and nothing appears.
 *
 * `pointer: coarse` hides it. There is no ⌘ on a phone, and the sentence about
 * ⌘V is about a keyboard the user does not have.
 */
interface QuickHintProps {
  /** ⌘1–⌘9 is inactive while a search is running (§3.5.3), so the hint that
   *  advertises it goes with it rather than lying. */
  searching: boolean;
}

export function QuickHint({ searching }: QuickHintProps) {
  return (
    <p className="hidden shrink-0 items-center justify-center gap-s-2 border-t border-divider bg-panel px-s-3 py-s-1 text-[11px] text-muted-foreground [@media(pointer:fine)]:flex">
      <span>
        <kbd className="font-sans">↑↓</kbd> move
      </span>
      <span aria-hidden="true">·</span>
      <span>
        <kbd className="font-sans">⏎</kbd> copy
      </span>
      {!searching && (
        <>
          <span aria-hidden="true">·</span>
          <span>
            <kbd className="font-sans">⌘1</kbd>–<kbd className="font-sans">⌘9</kbd>{" "}
            copy and close
          </span>
        </>
      )}
      <span aria-hidden="true">·</span>
      <span className="text-foreground">
        Copying puts the item on the clipboard — press{" "}
        <kbd className="font-sans">⌘V</kbd> yourself to paste
      </span>
    </p>
  );
}
