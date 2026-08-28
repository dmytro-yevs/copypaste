import { cloudConnectionPresentation } from "@/features/devices";
import type { CloudStatusData } from "@/lib/ipc";

export type CloudSettingsIcon =
  | "cloud"
  | "cloudOff"
  | "shieldCheck"
  | "alert";

export interface CloudSettingsPresentation {
  readonly icon: CloudSettingsIcon;
  readonly iconOwner: "device" | "settings";
}

export function cloudSettingsPresentation(
  status: CloudStatusData | undefined,
  queryFailed: boolean,
  loading: boolean,
): CloudSettingsPresentation {
  const device = () => ({
    icon: cloudConnectionPresentation(status, queryFailed, loading).icon,
    iconOwner: "device" as const,
  });
  const connected = Boolean(status?.signed_in && status.key_ready);
  const attention = Boolean(status?.last_error || status?.unreadable_uploads);

  if (loading) return device();
  if (queryFailed) return { icon: "alert", iconOwner: "settings" };
  if (connected && !attention) return device();
  if (connected) return { icon: "shieldCheck", iconOwner: "settings" };
  if (status?.configured) return device();
  return { icon: "cloud", iconOwner: "settings" };
}
