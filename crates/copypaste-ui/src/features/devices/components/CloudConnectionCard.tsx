import { Icon, type IconName } from "@/components/ui/icon";
import { SkeletonText } from "@/components/shared";

import { Button, Surface } from "@/components/ui";
import type { CloudStatusData } from "@/lib/ipc";
import styles from "./CloudConnectionCard.module.css";

type CloudState =
    "unavailable" | "not-configured" | "signed-out" | "attention" | "healthy";

function cloudPresentation(
    status: CloudStatusData | undefined,
    failed: boolean,
): { state: CloudState; detail: string } {
    if (failed || status === undefined) {
        return { state: "unavailable", detail: "Cloud status is unavailable." };
    }
    if (!status.configured) {
        return {
            state: "not-configured",
            detail: "Not configured · direct device sync still works",
        };
    }
    if (!status.signed_in || !status.key_ready) {
        return {
            state: "signed-out",
            detail: "Sign in from Settings to use encrypted cloud",
        };
    }
    if (status.last_error || status.unreadable_uploads > 0) {
        return {
            state: "attention",
            detail: "Encrypted cloud needs attention",
        };
    }
    return { state: "healthy", detail: "End-to-end encrypted · healthy" };
}

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
    if (loading) {
        return (
            <Surface
                elevation="raised"
                border="subtle"
                radius="md"
                tone="neutral"
                className={styles.card}
                data-state="checking"
                role="status"
                aria-label="Checking encrypted cloud"
                aria-busy="true"
            >
                <span className={styles.iconWell} aria-hidden="true">
                    <Icon name="cloud" size="md" />
                </span>
                <span className={styles.copy}>
                    <strong>Encrypted cloud</strong>
                    <SkeletonText width="md" className={styles.skeletonDetail} />
                </span>
                <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    className={styles.manage}
                    onClick={onManage}
                >
                    <Icon name="settings" size="sm" aria-hidden="true" />
                    <span>Manage</span>
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

    const presentation = cloudPresentation(status, failed);
    const icon: IconName =
        presentation.state === "healthy"
            ? "shieldCheck"
            : presentation.state === "attention"
              ? "alert"
              : "cloudOff";

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
                <Icon name={icon} size="md" />
            </span>
            <span className={styles.copy}>
                <strong>Encrypted cloud</strong>
                <small>{presentation.detail}</small>
            </span>
            <Button
                type="button"
                variant="secondary"
                size="sm"
                className={styles.manage}
                onClick={onManage}
            >
                <Icon name="settings" size="sm" aria-hidden="true" />
                <span>Manage</span>
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
