import { cloudConnectionState } from "@/features/devices/model";
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
  if (loading) return { icon: "cloud", iconOwner: "device" };
  if (queryFailed) return { icon: "alert", iconOwner: "settings" };
  const state = cloudConnectionState(status, false, false);
  if (state === "healthy") return { icon: "shieldCheck", iconOwner: "device" };
  if (state === "attention") return { icon: "shieldCheck", iconOwner: "settings" };
  if (state === "signed-out") return { icon: "cloudOff", iconOwner: "device" };
  return { icon: "cloud", iconOwner: "settings" };
}
