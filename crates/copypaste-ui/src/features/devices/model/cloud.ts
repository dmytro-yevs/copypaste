import { t } from "@/i18n";
import type { CloudStatusData } from "@/lib/ipc";

export type CloudConnectionState =
    | "checking"
    | "unavailable"
    | "not-configured"
    | "signed-out"
    | "attention"
    | "healthy";

export interface CloudConnectionPresentation {
    readonly state: CloudConnectionState;
    readonly icon: "cloud" | "cloudOff" | "shieldCheck" | "alert";
    readonly title: string;
    readonly detail?: string;
    readonly busy: boolean;
    readonly role: "status";
    readonly live: "polite";
    readonly action: { readonly label: string; readonly icon: "settings" };
}

export function cloudConnectionPresentation(
    status: CloudStatusData | undefined,
    failed: boolean,
    loading: boolean,
): CloudConnectionPresentation {
    const action = { label: t("devices.presentation.cloud.manage"), icon: "settings" as const };
    if (loading) {
        return {
            state: "checking",
            icon: "cloud",
            title: t("devices.presentation.cloud.title"),
            busy: true,
            role: "status",
            live: "polite",
            action,
        };
    }
    if (failed || status === undefined) return cloud("unavailable", "cloudOff", "unavailable", action);
    if (!status.configured) return cloud("not-configured", "cloudOff", "notConfigured", action);
    if (!status.signed_in || !status.key_ready) return cloud("signed-out", "cloudOff", "signedOut", action);
    if (status.last_error || status.unreadable_uploads > 0) return cloud("attention", "alert", "attention", action);
    return cloud("healthy", "shieldCheck", "healthy", action);
}

function cloud(
    state: Exclude<CloudConnectionState, "checking">,
    icon: Exclude<CloudConnectionPresentation["icon"], "cloud">,
    detail: "unavailable" | "notConfigured" | "signedOut" | "attention" | "healthy",
    action: CloudConnectionPresentation["action"],
): CloudConnectionPresentation {
    return {
        state,
        icon,
        title: t("devices.presentation.cloud.title"),
        detail: t(`devices.presentation.cloud.${detail}`),
        busy: false,
        role: "status",
        live: "polite",
        action,
    };
}
