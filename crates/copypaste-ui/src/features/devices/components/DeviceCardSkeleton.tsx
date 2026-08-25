import { Icon } from "@/components/ui";
import { SkeletonText } from "@/components/shared";
import { DeviceKindIcon } from "@/features/devices/components/DeviceKindIcon";
import type { DevicePresentationIdentity } from "@/features/devices/model/devicePresentation";
import styles from "./DeviceCardSkeleton.module.css";

export function DeviceCardSkeleton({
    label,
    identity,
    name,
    trustLabel,
}: {
    label: string;
    identity?: DevicePresentationIdentity;
    name?: string;
    trustLabel?: string;
}) {
    const knownIdentity = name !== undefined;

    return (
        <div
            className={styles.card}
            role="status"
            aria-label={label}
            aria-busy="true"
        >
            <span className={styles.identityWell} aria-hidden="true">
                {identity ? (
                    <DeviceKindIcon identity={identity} />
                ) : (
                    <Icon name="devices" size="md" />
                )}
            </span>
            <span className={styles.copy} aria-hidden="true">
                {knownIdentity ? (
                    <span className={styles.knownName}>{name}</span>
                ) : (
                    <SkeletonText width="sm" />
                )}
                {knownIdentity ? (
                    <span className={styles.knownDetail}>
                        <span>{trustLabel}</span>
                        <span className={styles.separator}>·</span>
                        <SkeletonText width="xs" />
                    </span>
                ) : (
                    <SkeletonText width="md" />
                )}
            </span>
            <Icon
                name="caretRight"
                size="sm"
                className={styles.chevron}
                aria-hidden="true"
            />
        </div>
    );
}
