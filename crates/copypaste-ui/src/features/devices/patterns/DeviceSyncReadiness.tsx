import { StateView } from "@/components/shared/StateView";
import { Button } from "@/components/ui";
import {
    syncReadinessIsLoading,
    syncReadinessMessage,
    syncReadinessOf,
    syncReadinessRecovery,
    type SyncReadiness,
    type SyncReadinessInput,
} from "@/features/devices/model";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";

type RecoverableSyncSources = SyncReadinessInput & {
    readonly service: { readonly refetch: () => unknown };
    readonly config: { readonly refetch: () => unknown };
    readonly peers: { readonly refetch: () => unknown };
};

export function useDeviceSyncReadiness(sources: RecoverableSyncSources) {
    const setView = useUi((state) => state.setView);
    const setSettingsTab = useUi((state) => state.setSettingsTab);
    const readiness = syncReadinessOf(sources);
    const recovery = syncReadinessRecovery(readiness);

    const recover = () => {
        switch (recovery) {
            case "retry-service":
                void sources.service.refetch();
                break;
            case "retry-config":
                void sources.config.refetch();
                break;
            case "retry-peers":
                void sources.peers.refetch();
                break;
            case "enable-sync":
                setSettingsTab("device-sync");
                setView("settings");
                break;
        }
    };

    return { readiness, recover };
}

interface DeviceSyncReadinessNoticeProps {
    readonly readiness: SyncReadiness;
    readonly onRecover: () => void;
}

export function DeviceSyncReadinessNotice({
    readiness,
    onRecover,
}: DeviceSyncReadinessNoticeProps) {
    const { t } = useTranslation();
    if (readiness === "ready" || readiness === "no-peers") return null;

    const loading = syncReadinessIsLoading(readiness);
    const recovery = syncReadinessRecovery(readiness);
    return (
        <StateView
            mode={loading ? "loading" : readiness === "disabled" ? "offline" : "warning"}
            placement="panel"
            title={syncReadinessMessage(readiness)}
            aria-busy={loading || undefined}
            actions={recovery === null ? undefined : (
                <Button variant="secondary" size="sm" onClick={onRecover}>
                    {recovery === "enable-sync"
                        ? t("devices.syncReadiness.openSettings")
                        : t("common.tryAgain")}
                </Button>
            )}
        />
    );
}
