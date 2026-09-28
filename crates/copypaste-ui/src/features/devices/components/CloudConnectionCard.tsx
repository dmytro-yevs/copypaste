import { StateView } from "@/components/shared/StateView";

import { Button } from "@/components/ui";
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
        <StateView
            mode={cloudStateMode(presentation.state)}
            placement="panel"
            title={presentation.title}
            description={presentation.detail}
            icon={presentation.icon}
            role={presentation.role}
            aria-live={presentation.live}
            aria-label={presentation.title}
            aria-busy={presentation.busy || undefined}
            actions={
                <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    icon={presentation.action.icon}
                    onClick={onManage}
                >
                    {presentation.action.label}
                </Button>
            }
        />
    );
}

function cloudStateMode(state: ReturnType<typeof cloudConnectionPresentation>["state"]) {
    switch (state) {
        case "checking": return "loading" as const;
        case "unavailable": return "error" as const;
        case "not-configured": return "offline" as const;
        case "signed-out":
        case "attention": return "warning" as const;
        case "healthy": return "success" as const;
    }
}
