// ---------------------------------------------------------------------------
// Reference-counted body scroll-lock (design.md Decision 5 — the one genuinely
// new behavior of the Dialog primitive). A shared counter so stacked/nested
// dialogs don't let the inner dialog's cleanup restore body scroll while the
// outer one is still open. Only the last dialog to close restores the original
// overflow value.
// ---------------------------------------------------------------------------
let scrollLockCount = 0;
let savedBodyOverflow = "";

export function acquireScrollLock(): void {
  if (scrollLockCount === 0) {
    savedBodyOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
  }
  scrollLockCount += 1;
}

export function releaseScrollLock(): void {
  scrollLockCount = Math.max(0, scrollLockCount - 1);
  if (scrollLockCount === 0) {
    document.body.style.overflow = savedBodyOverflow;
  }
}

/** Current lock depth — for assertions/tests only. */
export function scrollLockDepth(): number {
  return scrollLockCount;
}

/** Test-only reset of the shared counter between test cases. */
export function __resetScrollLockForTests(): void {
  scrollLockCount = 0;
  savedBodyOverflow = "";
  document.body.style.overflow = "";
}
