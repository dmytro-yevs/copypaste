import type { DeviceClass, DevicePlatform } from "@/lib/ipc";

export type DevicePresentationIcon =
  | "devices"
  | "monitor"
  | "laptop"
  | "mobile"
  | "tablet";

export interface DevicePresentationIdentity {
  readonly platform: DevicePlatform;
  readonly formFactor: DeviceClass;
  readonly source: "native-local" | "peer-asserted" | "unknown";
}

export interface DeviceClassDescriptor {
  readonly icon: DevicePresentationIcon;
}

export const UNKNOWN_DEVICE_IDENTITY: DevicePresentationIdentity = {
  platform: "unknown",
  formFactor: "unknown",
  source: "unknown",
};

const descriptors: Record<DeviceClass, DeviceClassDescriptor> = {
  desktop: { icon: "monitor" },
  laptop: { icon: "laptop" },
  phone: { icon: "mobile" },
  tablet: { icon: "tablet" },
  unknown: { icon: "devices" },
};

export function deviceClassDescriptor(
  formFactor: DeviceClass,
): DeviceClassDescriptor {
  return descriptors[formFactor];
}

export function observedDeviceIdentity(
  profile: Pick<DeviceProfile, "platform" | "device_class"> | null | undefined,
): DevicePresentationIdentity {
  return profile
    ? {
        platform: profile.platform,
        formFactor: profile.device_class,
        source: "peer-asserted",
      }
    : UNKNOWN_DEVICE_IDENTITY;
}

interface DeviceProfile {
  readonly platform: DevicePlatform;
  readonly device_class: DeviceClass;
}
