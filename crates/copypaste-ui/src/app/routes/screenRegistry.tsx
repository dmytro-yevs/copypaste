import { lazy, type ReactNode } from "react";

import type { View } from "@/store/ui";

const LibraryScreen = lazy(async () => ({ default: (await import("@/features/history")).LibraryScreen }));
const DevicesScreen = lazy(async () => ({ default: (await import("@/features/devices")).DevicesScreen }));
const SettingsScreen = lazy(async () => ({ default: (await import("@/features/settings")).SettingsScreen }));
const CaptureScreen = lazy(async () => ({ default: (await import("@/features/capture")).CaptureScreen }));

interface ScreenDefinition {
  label: "nav.history" | "nav.connections" | "nav.preferences" | "capture.title";
  render: (pushLive: boolean) => ReactNode;
}

export const screenRegistry: Record<View, ScreenDefinition> = {
  history: { label: "nav.history", render: (pushLive) => <LibraryScreen pushLive={pushLive} /> },
  devices: { label: "nav.connections", render: () => <DevicesScreen /> },
  settings: { label: "nav.preferences", render: () => <SettingsScreen /> },
  capture: { label: "capture.title", render: () => <CaptureScreen /> },
};
