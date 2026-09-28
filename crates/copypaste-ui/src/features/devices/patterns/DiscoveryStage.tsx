import type { ReactNode } from "react";

import { StateView } from "@/components/shared/StateView";
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
                <StateView
                    mode={state === "error" ? "error" : state === "checking" || state === "scanning" ? "loading" : "info"}
                    placement="panel"
                    title={presentation.title}
                    description={presentation.body}
                    icon={presentation.icon ?? undefined}
                    aria-busy={presentation.busy || undefined}
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
            <StateView
                mode={refreshing ? "loading" : "info"}
                placement="panel"
                title={results.label}
                description={refreshing ? t("devices.discovered.refreshing") : results.detail}
                icon={refreshing ? undefined : "devices"}
                role="status"
                aria-live="polite"
                aria-atomic="true"
                aria-busy={refreshing || undefined}
            />
            <div className={styles.results}>{children}</div>
        </div>
    );
}
