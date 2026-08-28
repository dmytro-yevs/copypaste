import { Button, Icon } from "@/components/ui";
import { DeviceKindIcon } from "@/features/devices/components/DeviceKindIcon";
import {
    discoveredDeviceIdentity,
    discoveredStatus,
} from "@/features/devices/model/devicePresentation";
import { t } from "@/i18n";
import type { DiscoveredDevice } from "@/lib/ipc";
import styles from "./DiscoveryDeviceCard.module.css";

export function DiscoveryDeviceCard({
    device,
    selected,
    onSelect,
}: {
    device: DiscoveredDevice;
    selected: boolean;
    onSelect: () => void;
}) {
    const status = discoveredStatus(device);
    const identity = discoveredDeviceIdentity(device);

    return (
        <Button
            type="button"
            variant="ghost"
            size="md"
            className={styles.root}
            data-selected={selected || undefined}
            aria-expanded={selected}
            aria-label={t("devices.presentation.discoveryCard.ariaLabel", {
                name: device.name,
                status: status.label,
            })}
            data-device-selection-key={`discovered:${device.discovery_id}`}
            onClick={onSelect}
        >
            <span className={styles.identityWell} aria-hidden="true">
                <DeviceKindIcon identity={identity} />
            </span>
            <span className={styles.copy}>
                <strong>{device.name}</strong>
                <span className={styles.metaLine}>
                    <span>{t("devices.presentation.discoveryCard.subtitle")}</span>
                </span>
            </span>
            <Icon name="caretRight" size="sm" className={styles.chevron} />
        </Button>
    );
}
