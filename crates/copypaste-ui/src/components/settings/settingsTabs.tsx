import {
  CircleHelp,
  ClipboardCheck,
  Database,
  Keyboard,
  List,
  MonitorSmartphone,
  Palette,
  Settings2,
  Stethoscope,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { ReactElement } from "react";

import { AboutTab } from "@/components/settings/AboutTab";
import { AppearanceTab } from "@/components/settings/AppearanceTab";
import { DiagnosticsTab } from "@/components/settings/DiagnosticsTab";
import { ListTab } from "@/components/settings/ListTab";
import { ServiceTab } from "@/components/settings/ServiceTab";
import { ShortcutTab } from "@/components/settings/ShortcutTab";
import { StorageTab } from "@/components/settings/StorageTab";
import { SyncTab } from "@/components/settings/SyncTab";
import { CaptureSetup } from "@/components/capture/CaptureSetup";

export const TABS = [
  { value: "appearance", label: "settings.tabs.appearance", icon: Palette, render: () => <AppearanceTab /> },
  { value: "list", label: "settings.tabs.list", icon: List, render: () => <ListTab /> },
  { value: "shortcut", label: "settings.tabs.shortcut", icon: Keyboard, render: () => <ShortcutTab /> },
  { value: "service", label: "settings.tabs.service", icon: Settings2, render: () => <ServiceTab /> },
  { value: "capture", label: "capture.title", icon: ClipboardCheck, render: () => <CaptureSetup /> },
  { value: "sync", label: "settings.tabs.sync", icon: MonitorSmartphone, render: () => <SyncTab /> },
  { value: "storage", label: "settings.tabs.storage", icon: Database, render: () => <StorageTab /> },
  { value: "diagnostics", label: "settings.tabs.diagnostics", icon: Stethoscope, render: () => <DiagnosticsTab /> },
  { value: "about", label: "settings.tabs.about", icon: CircleHelp, render: () => <AboutTab /> },
] as const satisfies ReadonlyArray<{
  value: string;
  label: string;
  icon: LucideIcon;
  render: () => ReactElement;
}>;

export type SettingsTab = (typeof TABS)[number];

type GroupLabel =
  | "settings.groups.personal"
  | "settings.groups.service"
  | "settings.groups.support";

const GROUPS = [
  { label: "settings.groups.personal", tabs: ["appearance", "list", "shortcut"] },
  { label: "settings.groups.service", tabs: ["service", "capture", "sync", "storage"] },
  { label: "settings.groups.support", tabs: ["diagnostics", "about"] },
] as const satisfies ReadonlyArray<{
  label: GroupLabel;
  tabs: readonly SettingsTab["value"][];
}>;

export interface SettingsGroup {
  label: GroupLabel;
  tabs: readonly SettingsTab[];
}

/** Android has its own capture setup, but Service and Storage expose controls
 *  backed by native commands on every product platform. */
export function visibleTabs(android: boolean): readonly SettingsTab[] {
  return android
    ? TABS.filter((tab) => tab.value !== "shortcut")
    : TABS.filter((tab) => tab.value !== "capture");
}

/** Grouping follows what is visible, so a group whose whole membership is
 *  absent on this platform does not render an empty heading. */
export function groupedTabs(tabs: readonly SettingsTab[]): readonly SettingsGroup[] {
  return GROUPS.map((group) => ({
    label: group.label,
    tabs: group.tabs.flatMap((value) => tabs.filter((tab) => tab.value === value)),
  })).filter((group) => group.tabs.length > 0);
}
