/**
 * Every user-facing string in this app, behind one `t`.
 *
 * # Daemon text is not a message
 *
 * A failure crossing the bridge carries the daemon's own already-worded
 * sentence, and that sentence can name the socket path (INV-12). It is
 * therefore **classified and discarded**, never wrapped: `classifyError` turns
 * it into an `ErrorKind` token, and `errors.<kind>` is the one authored
 * sentence for that kind. Nothing in the catalogue interpolates daemon text,
 * so there is no path by which a message can be worded twice or leak a path.
 *
 * # Clip content never reaches a template
 *
 * No catalogue entry takes a clip as an interpolation variable. Where an
 * accessible name has to contain the clip — the row and its checkbox — the
 * catalogue supplies a prefix and the caller concatenates, so the content is
 * never an argument to `t`. `catalogue.test.ts` enforces both rules.
 *
 * # One language
 *
 * English only, so components call the module-scope `t` freely; `useTranslation`
 * exists for the render-on-language-change a second language would need, and
 * `Trans` for the messages that embed a `<kbd>`.
 */
import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import { en } from "@/i18n/en";

export const DEFAULT_NS = "app";

void i18n.use(initReactI18next).init({
  lng: "en",
  fallbackLng: "en",
  ns: [DEFAULT_NS],
  defaultNS: DEFAULT_NS,
  resources: { en: { [DEFAULT_NS]: en } },
  // React escapes on render; escaping again here turns a peer's `&` into
  // `&amp;` on screen.
  interpolation: { escapeValue: false },
});

export const t = i18n.t;

export { Trans, useTranslation } from "react-i18next";
export { en } from "@/i18n/en";
export default i18n;
