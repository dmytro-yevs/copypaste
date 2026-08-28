import {
    MetadataLabel,
    MetadataList,
    MetadataRow,
    MetadataValue,
} from "@/components/shared";
import { DeviceStatus } from "@/features/devices/components/DeviceStatus";
import {
    discoveredDeviceDetails,
    type DeviceStatusPresentation,
} from "@/features/devices/model";
import { t } from "@/i18n";
import type { DiscoveredDevice } from "@/lib/ipc";
import styles from "./DiscoveredDeviceDetails.module.css";

export function DiscoveredDeviceDetails({
    device,
    status,
}: {
    device: DiscoveredDevice;
    status: DeviceStatusPresentation;
}) {
    const groups = discoveredDeviceDetails(device);

    return (
        <div className={styles.groups}>
            {groups.map((group) => (
                <section key={group.label} className={styles.group}>
                    <h3>{group.label}</h3>
                    <MetadataList className={styles.list}>
                        {group.includesStatus ? (
                            <MetadataRow>
                                <MetadataLabel>{t("devices.detail.status")}</MetadataLabel>
                                <MetadataValue><DeviceStatus status={status} /></MetadataValue>
                            </MetadataRow>
                        ) : null}
                        {group.rows.map(([term, value]) => (
                            <MetadataRow key={term}>
                                <MetadataLabel>{term}</MetadataLabel>
                                <MetadataValue>{value}</MetadataValue>
                            </MetadataRow>
                        ))}
                    </MetadataList>
                </section>
            ))}
        </div>
    );
}
