import { capture } from "@/i18n/en/capture";
import { common, errors, nav } from "@/i18n/en/common";
import { devices } from "@/i18n/en/devices";
import { history } from "@/i18n/en/history";
import { settings } from "@/i18n/en/settings";
import { shell } from "@/i18n/en/shell";
import { tray } from "@/i18n/en/tray";

export const en = {
  common,
  nav,
  errors,
  shell,
  history,
  devices,
  settings,
  capture,
  tray,
} as const;

export type Catalogue = typeof en;
