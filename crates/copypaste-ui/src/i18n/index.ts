/**
 * Two rules, both enforced by `catalogue.test.ts`:
 *
 * Daemon text is never interpolated into a message. It can name the socket path
 * (INV-12), so the structured code selects `errors.<kind>`
 * is the one authored sentence for that kind.
 *
 * Clip content is never an argument to `t`. Where an accessible name has to
 * contain a clip, the catalogue supplies a prefix and the caller concatenates.
 *
 * English only, so components may call the module-scope `t` freely;
 * `useTranslation` exists for the re-render a second language would need, and
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
