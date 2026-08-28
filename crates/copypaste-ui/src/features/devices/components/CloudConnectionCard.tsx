import { Icon } from "@/components/ui/icon";
import { SkeletonText } from "@/components/shared";

import { Button, Surface } from "@/components/ui";
import { cloudConnectionPresentation } from "@/features/devices/model";
import type { CloudStatusData } from "@/lib/ipc";
import styles from "./CloudConnectionCard.module.css";

export function CloudConnectionCard({
    status,
    loading,
    failed,
    onManage,
}: {
    status: CloudStatusData | undefined;
    loading: boolean;
    failed: boolean;
    onManage: () => void;
}) {
    const presentation = cloudConnectionPresentation(status, failed, loading);
    if (presentation.state === "checking") {
        return (
            <Surface
                elevation="raised"
                border="subtle"
                radius="md"
                tone="neutral"
                className={styles.card}
                data-state="checking"
                role="status"
                aria-label={presentation.title}
                aria-busy={presentation.busy}
            >
                <span className={styles.iconWell} aria-hidden="true">
                    <Icon name="cloud" size="md" />
                </span>
                <span className={styles.copy}>
                    <strong>{presentation.title}</strong>
                    <SkeletonText width="md" className={styles.skeletonDetail} />
                </span>
                <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    className={styles.manage}
                    onClick={onManage}
                >
                    <Icon name={presentation.action.icon} size="sm" aria-hidden="true" />
                    <span>{presentation.action.label}</span>
                    <Icon
                        name="caretRight"
                        size="sm"
                        className={styles.manageIcon}
                        aria-hidden="true"
                    />
                </Button>
            </Surface>
        );
    }

    return (
        <Surface
            elevation="raised"
            border="subtle"
            radius="md"
            tone="neutral"
            className={styles.card}
            data-state={presentation.state}
        >
            <span className={styles.iconWell} aria-hidden="true">
                <Icon name={presentation.icon} size="md" />
            </span>
            <span className={styles.copy}>
                <strong>{presentation.title}</strong>
                <small>{presentation.detail}</small>
            </span>
            <Button
                type="button"
                variant="secondary"
                size="sm"
                className={styles.manage}
                onClick={onManage}
            >
                <Icon name={presentation.action.icon} size="sm" aria-hidden="true" />
                <span>{presentation.action.label}</span>
                <Icon
                    name="caretRight"
                    size="sm"
                    className={styles.manageIcon}
                    aria-hidden="true"
                />
            </Button>
        </Surface>
    );
}
