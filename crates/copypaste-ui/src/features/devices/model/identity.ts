import { t } from "@/i18n";
import {
    UNKNOWN_DEVICE_IDENTITY,
    deviceClassDescriptor,
    observedDeviceIdentity,
    type DevicePresentationIcon,
    type DevicePresentationIdentity,
} from "@/lib/deviceIdentity";
import type {
    DeviceClass,
    DevicePlatform,
    DiscoveredDevice,
    PeerInfo,
} from "@/lib/ipc";

export type {
    DevicePresentationIcon,
    DevicePresentationIdentity,
};
export { UNKNOWN_DEVICE_IDENTITY };

export interface DeviceIdentityDescriptor {
    readonly icon: DevicePresentationIcon;
    readonly label: string;
}

export const DEVICE_PLATFORM_LABELS = {
    macos: t("devices.presentation.platform.macos"),
    windows: t("devices.presentation.platform.windows"),
    android: t("devices.presentation.platform.android"),
    unknown: t("devices.presentation.platform.unknown"),
} as const satisfies Record<DevicePlatform, string>;

export const DEVICE_FORM_FACTOR_LABELS = {
    desktop: t("devices.presentation.class.desktop"),
    laptop: t("devices.presentation.class.laptop"),
    phone: t("devices.presentation.class.phone"),
    tablet: t("devices.presentation.class.tablet"),
    unknown: t("devices.presentation.class.unknown"),
} as const satisfies Record<DeviceClass, string>;

const identityDescriptors: Record<DeviceClass, DeviceIdentityDescriptor> = {
    desktop: {
        icon: deviceClassDescriptor("desktop").icon,
        label: DEVICE_FORM_FACTOR_LABELS.desktop,
    },
    laptop: {
        icon: deviceClassDescriptor("laptop").icon,
        label: DEVICE_FORM_FACTOR_LABELS.laptop,
    },
    phone: {
        icon: deviceClassDescriptor("phone").icon,
        label: DEVICE_FORM_FACTOR_LABELS.phone,
    },
    tablet: {
        icon: deviceClassDescriptor("tablet").icon,
        label: DEVICE_FORM_FACTOR_LABELS.tablet,
    },
    unknown: {
        icon: deviceClassDescriptor("unknown").icon,
        label: DEVICE_FORM_FACTOR_LABELS.unknown,
    },
};

export function deviceIdentityDescriptor(
    identity: DevicePresentationIdentity,
): DeviceIdentityDescriptor {
    return identityDescriptors[identity.formFactor];
}

export function deviceIconKind(
    identity: DevicePresentationIdentity,
): DevicePresentationIcon {
    return deviceClassDescriptor(identity.formFactor).icon;
}

export function discoveredDeviceIdentity(
    device: DiscoveredDevice,
): DevicePresentationIdentity {
    return observedDeviceIdentity(device.details?.profile);
}

export function peerIdentity(peer: PeerInfo): DevicePresentationIdentity {
    return observedDeviceIdentity(peer.details?.profile);
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
