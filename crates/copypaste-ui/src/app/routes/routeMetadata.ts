import type { IconName } from "@/components/ui";
import type { View } from "@/store/ui";

type RouteLabel = "nav.history" | "nav.devices" | "nav.settings" | "capture.title";
type NavigationSurface = "sidebar" | "dock";

export interface RouteMetadata {
  readonly label: RouteLabel;
  readonly icon: IconName;
  readonly navigation?: Readonly<Partial<Record<NavigationSurface, number>>>;
}

export const routeMetadata = {
  history: {
    label: "nav.history",
    icon: "library",
    navigation: { sidebar: 0, dock: 1 },
  },
  devices: {
    label: "nav.devices",
    icon: "devices",
    navigation: { sidebar: 1, dock: 0 },
  },
  settings: {
    label: "nav.settings",
    icon: "settings",
    navigation: { sidebar: 2, dock: 2 },
  },
  capture: { label: "capture.title", icon: "copy" },
} as const satisfies Record<View, RouteMetadata>;

export function navigationRoutes(surface: NavigationSurface) {
  return (Object.entries(routeMetadata) as Array<[View, RouteMetadata]>)
    .filter(([, route]) => route.navigation?.[surface] !== undefined)
    .sort(([, left], [, right]) =>
      (left.navigation?.[surface] ?? Number.POSITIVE_INFINITY)
      - (right.navigation?.[surface] ?? Number.POSITIVE_INFINITY),
    )
    .map(([view, route]) => ({ view, label: route.label, icon: route.icon }));
}
