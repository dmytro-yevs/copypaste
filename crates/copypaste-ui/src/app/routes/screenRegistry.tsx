import { lazy, type ComponentType, type ReactNode } from "react";

import type { View } from "@/store/ui";
import { routeMetadata, type RouteMetadata } from "./routeMetadata";

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
  render: (pushLive: boolean) => ReactNode;
  reset: () => void;
}

function createLazyScreen<Props extends object>(loader: () => Promise<{ default: ComponentType<Props> }>) {
  let LazyScreen = lazy(loader);

  function Screen(props: Props) {
    const CurrentScreen = LazyScreen;
    return <CurrentScreen {...props} />;
  }

  return {
    render: (props: Props) => <Screen {...props} />,
    reset: () => {
      LazyScreen = lazy(loader);
    },
  };
}

function createScreenRegistry(loaders: ScreenLoaders): Record<View, RouteMetadata & ScreenDefinition> {
  const history = createLazyScreen(loaders.history);
  const devices = createLazyScreen(loaders.devices);
  const settings = createLazyScreen(loaders.settings);
  const capture = createLazyScreen(loaders.capture);
  return {
    history: { ...routeMetadata.history, render: (pushLive) => history.render({ pushLive }), reset: history.reset },
    devices: { ...routeMetadata.devices, render: () => devices.render({}), reset: devices.reset },
    settings: { ...routeMetadata.settings, render: () => settings.render({}), reset: settings.reset },
    capture: { ...routeMetadata.capture, render: () => capture.render({}), reset: capture.reset },
  } satisfies Record<View, RouteMetadata & ScreenDefinition>;
}

export const screenRegistry = createScreenRegistry(screenLoaders);
