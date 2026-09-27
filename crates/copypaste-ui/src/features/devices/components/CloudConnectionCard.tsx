import { SkeletonText, StatusCard, type StatusCardStatus } from "@/components/shared";

import { Button, Icon } from "@/components/ui";
import { cloudConnectionPresentation } from "@/features/devices/model";
import type { CloudStatusData } from "@/lib/ipc";

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
    return (
        <StatusCard
            status={statusOf(presentation.state)}
            title={presentation.title}
            detail={presentation.state === "checking"
                ? <SkeletonText width="md" />
                : presentation.detail}
            icon={presentation.icon}
            variant="prominent"
            role={presentation.role}
            live={presentation.live}
            aria-label={presentation.title}
            busy={presentation.busy}
            action={
                <Button type="button" variant="secondary" size="sm" onClick={onManage}>
                    <Icon name={presentation.action.icon} size="sm" aria-hidden="true" />
                    <span>{presentation.action.label}</span>
                </Button>
            }
        />
    );
}

function statusOf(state: ReturnType<typeof cloudConnectionPresentation>["state"]): StatusCardStatus {
    if (state === "healthy") return "positive";
    if (state === "attention" || state === "unavailable") return "danger";
    return "neutral";
}
