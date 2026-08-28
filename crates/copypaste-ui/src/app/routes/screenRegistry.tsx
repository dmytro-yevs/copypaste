import { lazy, type ComponentType, type ReactNode } from "react";

import type { View } from "@/store/ui";

interface ScreenLoaders {
  history: () => Promise<{ default: ComponentType<{ pushLive?: boolean }> }>;
  devices: () => Promise<{ default: ComponentType }>;
  settings: () => Promise<{ default: ComponentType }>;
  capture: () => Promise<{ default: ComponentType }>;
}

const screenLoaders: ScreenLoaders = {
  history: async () => ({ default: (await import("@/features/history")).LibraryScreen }),
  devices: async () => ({ default: (await import("@/features/devices")).DevicesScreen }),
  settings: async () => ({ default: (await import("@/features/settings")).SettingsScreen }),
  capture: async () => ({ default: (await import("@/features/capture")).CaptureScreen }),
};

interface ScreenDefinition {
  label: "nav.history" | "nav.connections" | "nav.preferences" | "capture.title";
  render: (pushLive: boolean) => ReactNode;
}

export function createScreenRegistry(loaders: ScreenLoaders): Record<View, ScreenDefinition> {
  const LibraryScreen = lazy(loaders.history);
  const DevicesScreen = lazy(loaders.devices);
  const SettingsScreen = lazy(loaders.settings);
  const CaptureScreen = lazy(loaders.capture);

  return {
    history: { label: "nav.history", render: (pushLive) => <LibraryScreen pushLive={pushLive} /> },
    devices: { label: "nav.connections", render: () => <DevicesScreen /> },
    settings: { label: "nav.preferences", render: () => <SettingsScreen /> },
    capture: { label: "capture.title", render: () => <CaptureScreen /> },
  };
}

export const screenRegistry = createScreenRegistry(screenLoaders);
