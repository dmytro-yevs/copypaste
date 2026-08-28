import { t } from "@/i18n";
import type {
    DeviceClass,
    DevicePlatform,
    DiscoveredDevice,
} from "@/lib/ipc";

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

export interface DeviceIdentityDescriptor {
    readonly icon: DevicePresentationIcon;
    readonly label: string;
}

export const UNKNOWN_DEVICE_IDENTITY: DevicePresentationIdentity = {
    platform: "unknown",
    formFactor: "unknown",
    source: "unknown",
};

const identityDescriptors: Record<DeviceClass, DeviceIdentityDescriptor> = {
    desktop: { icon: "monitor", label: t("devices.presentation.class.desktop") },
    laptop: { icon: "laptop", label: t("devices.presentation.class.laptop") },
    phone: { icon: "mobile", label: t("devices.presentation.class.phone") },
    tablet: { icon: "tablet", label: t("devices.presentation.class.tablet") },
    unknown: { icon: "devices", label: t("devices.presentation.class.unknown") },
};

export const DEVICE_PLATFORM_LABELS = {
    macos: t("devices.presentation.platform.macos"),
    windows: t("devices.presentation.platform.windows"),
    android: t("devices.presentation.platform.android"),
    unknown: t("devices.presentation.platform.unknown"),
} as const satisfies Record<DevicePlatform, string>;

export const DEVICE_FORM_FACTOR_LABELS = {
    desktop: identityDescriptors.desktop.label,
    laptop: identityDescriptors.laptop.label,
    phone: identityDescriptors.phone.label,
    tablet: identityDescriptors.tablet.label,
    unknown: identityDescriptors.unknown.label,
} as const satisfies Record<DeviceClass, string>;

export function deviceIdentityDescriptor(
    identity: DevicePresentationIdentity,
): DeviceIdentityDescriptor {
    return identityDescriptors[identity.formFactor];
}

export function deviceIconKind(
    identity: DevicePresentationIdentity,
): DevicePresentationIcon {
    return deviceIdentityDescriptor(identity).icon;
}

export function discoveredDeviceIdentity(
    device: DiscoveredDevice,
): DevicePresentationIdentity {
    const profile = device.details?.profile;
    return profile
        ? {
              platform: profile.platform,
              formFactor: profile.device_class,
              source: "peer-asserted",
          }
        : UNKNOWN_DEVICE_IDENTITY;
}

export function localDeviceIdentity(
    platform: string,
): DevicePresentationIdentity {
    if (
        platform === "android" ||
        platform === "macos" ||
        platform === "windows"
    ) {
        return { platform, formFactor: "unknown", source: "native-local" };
    }
    return UNKNOWN_DEVICE_IDENTITY;
}
