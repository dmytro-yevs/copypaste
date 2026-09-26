import {
  cloudConnectionState,
  type CloudConnectionState,
} from "@/features/devices/model";
import type { CloudStatusData } from "@/lib/ipc";

type CloudSettingsTextKey =
  | "settings.sync.cloud.loading"
  | "settings.sync.cloud.statusUnavailable"
  | "settings.sync.cloud.notConfigured"
  | "settings.sync.cloud.description"
  | "settings.sync.cloud.signedOutDescription"
  | "settings.sync.cloud.attentionDescription"
  | "settings.sync.cloud.badgeNotConfigured"
  | "settings.sync.cloud.badgeSignedOut"
  | "settings.sync.cloud.badgeAttention"
  | "settings.sync.cloud.badgeConnected";

export interface CloudSettingsPresentation {
  readonly state: CloudConnectionState;
  readonly icon: "cloud" | "cloudOff" | "shieldCheck" | "alert";
  readonly description: CloudSettingsTextKey;
  readonly badge?: {
    readonly label: CloudSettingsTextKey;
    readonly variant: "warn" | "secondary" | "ok";
  };
}

export function cloudSettingsPresentation(
  status: CloudStatusData | undefined,
  queryFailed: boolean,
  loading: boolean,
  syncFailed = false,
): CloudSettingsPresentation {
  const state = cloudConnectionState(status, queryFailed, loading, syncFailed);
  switch (state) {
    case "checking":
      return { state, icon: "cloud", description: "settings.sync.cloud.loading" };
    case "unavailable":
      return { state, icon: "cloudOff", description: "settings.sync.cloud.statusUnavailable" };
    case "not-configured":
      return {
        state,
        icon: "cloudOff",
        description: "settings.sync.cloud.notConfigured",
        badge: { label: "settings.sync.cloud.badgeNotConfigured", variant: "warn" },
      };
    case "signed-out":
      return {
        state,
        icon: "cloudOff",
        description: "settings.sync.cloud.signedOutDescription",
        badge: { label: "settings.sync.cloud.badgeSignedOut", variant: "secondary" },
      };
    case "attention":
      return {
        state,
        icon: "alert",
        description: "settings.sync.cloud.attentionDescription",
        badge: { label: "settings.sync.cloud.badgeAttention", variant: "warn" },
      };
    case "healthy":
      return {
        state,
        icon: "shieldCheck",
        description: "settings.sync.cloud.description",
        badge: { label: "settings.sync.cloud.badgeConnected", variant: "ok" },
      };
  }
}
