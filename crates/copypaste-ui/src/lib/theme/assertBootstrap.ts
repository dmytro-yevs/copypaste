/**
 * Assert the pre-paint `theme-bootstrap.js` ran before this module (task 1.15).
 *
 * The bootstrap sets `document.documentElement.dataset.themeBootstrapped = "1"`
 * synchronously in <head>, before the module entry. Called at module-eval time,
 * this reads that marker to prove script ORDER without needing pixel-level paint
 * timing. In DEV a missing marker is a loud console warning (misconfigured HTML);
 * the packaged-Tauri smoke test (Slice 6) upgrades it to a hard release gate.
 * Returns whether the marker was present so callers/tests can assert on it.
 */
export function assertBootstrapRanBeforeModule(entry: string): boolean {
  const ran =
    typeof document !== "undefined" &&
    document.documentElement.dataset.themeBootstrapped === "1";

  if (!ran && import.meta.env.DEV) {
    console.warn(
      `[theme] ${entry} entry started but theme-bootstrap.js had not run ` +
        `(dataset.themeBootstrapped missing). The pre-paint <script src=` +
        `"./theme-bootstrap.js"> must precede the module entry in the HTML.`,
    );
  }
  return ran;
}
