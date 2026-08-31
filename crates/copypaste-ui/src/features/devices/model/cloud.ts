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

export function cloudConnectionState(
    status: CloudStatusData | undefined,
    failed: boolean,
    loading: boolean,
): CloudConnectionState {
    if (loading) return "checking";
    if (failed || status === undefined) return "unavailable";
    if (!status.configured) return "not-configured";
    if (!status.signed_in || !status.key_ready) return "signed-out";
    if (status.last_error || status.unreadable_uploads > 0) return "attention";
    return "healthy";
}

export function cloudConnectionPresentation(
    status: CloudStatusData | undefined,
    failed: boolean,
    loading: boolean,
): CloudConnectionPresentation {
    const action = { label: t("devices.presentation.cloud.manage"), icon: "settings" as const };
    const state = cloudConnectionState(status, failed, loading);
    if (state === "checking") {
        return {
            state,
            icon: "cloud",
            title: t("devices.presentation.cloud.title"),
            busy: true,
            role: "status",
            live: "polite",
            action,
        };
    }
    if (state === "unavailable") return cloud(state, "cloudOff", "unavailable", action);
    if (state === "not-configured") return cloud(state, "cloudOff", "notConfigured", action);
    if (state === "signed-out") return cloud(state, "cloudOff", "signedOut", action);
    if (state === "attention") return cloud(state, "alert", "attention", action);
    return cloud(state, "shieldCheck", "healthy", action);
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
