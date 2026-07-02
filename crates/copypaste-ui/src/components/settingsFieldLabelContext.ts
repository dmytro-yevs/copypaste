import { createContext } from "react";

// g27b.26 — a11y accessible-name plumbing.
//
// Form controls rendered inside a SettingsRow (Toggle, SliderRow, bare inputs)
// are visually labelled by the row's title text, but that text lives in a
// sibling node — so the control itself has NO accessible name and axe flags
// `button-name` (toggles) / `label` (sliders/inputs) as critical.
//
// SettingsRow publishes its visible title through this context; Toggle and
// SliderRow read it as a FALLBACK aria-label (an explicit aria-label prop still
// wins). This fixes every settings control at once with no call-site churn and
// keeps the accessible name in sync with the visible label by construction.
export const SettingsFieldLabelContext = createContext<string | undefined>(
  undefined,
);
