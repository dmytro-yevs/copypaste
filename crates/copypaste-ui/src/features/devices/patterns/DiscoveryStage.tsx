import type { ReactNode } from "react";

import { EmptyState } from "@/components/shared";
import { Icon, Surface } from "@/components/ui";
import {
    discoveryResultsPresentation,
    discoveryStagePresentation,
    type DiscoveryStageState,
} from "@/features/devices/model";
import { t } from "@/i18n";
import styles from "./DiscoveryStage.module.css";

export type { DiscoveryStageState } from "@/features/devices/model";

interface DiscoveryStageProps {
    readonly state: DiscoveryStageState;
    readonly deviceCount: number;
    readonly refreshing?: boolean;
    readonly children: ReactNode;
}

/** Keeps the network list in normal document flow. A discovery refresh only
 * updates the summary, so it never replaces or moves a device the user chose. */
export function DiscoveryStage({
    state,
    deviceCount,
    refreshing = false,
    children,
}: DiscoveryStageProps) {
    const presentation = discoveryStagePresentation(state);
    const results = discoveryResultsPresentation(deviceCount);

    if (state !== "results") {
        return (
            <div className={styles.stage} data-state={state}>
                <EmptyState
                    compact
                    busy={presentation.busy}
                    tone={presentation.tone}
                    icon={presentation.icon ?? undefined}
                    title={presentation.title}
                    body={presentation.body}
                />
            </div>
        );
    }

    return (
        <div
            className={styles.stage}
            data-state={state}
            aria-busy={refreshing || undefined}
        >
            <Surface
                elevation="raised"
                border="subtle"
                radius="md"
                className={styles.summary}
                role="status"
                aria-live="polite"
                aria-atomic="true"
            >
                <span className={styles.summaryIcon} aria-hidden="true">
                    {refreshing ? (
                        <Icon name="spinner" className={styles.spinner} size="sm" />
                    ) : (
                        <Icon name="devices" size="sm" />
                    )}
                </span>
                <span className={styles.summaryCopy}>
                    <strong>{results.label}</strong>
                    <span>
                        {refreshing
                            ? t("devices.discovered.refreshing")
                            : results.detail}
                    </span>
                </span>
            </Surface>
            <div className={styles.results}>{children}</div>
        </div>
    );
}
