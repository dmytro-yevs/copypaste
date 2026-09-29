import { Button } from "@/components/ui";
import { PreviewSurface } from "@/components/shared";
import { StateView } from "@/components/shared/StateView";
import { ClipImageLoader } from "@/features/clip-content";
import { InspectorPreview } from "@/features/history/components/InspectorPreview";
import { useTranslation } from "@/i18n";
import type { ClipBodyPresentation as Body } from "@/lib/clipPresentation";
import type { Kind } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import inspectorStyles from "./LibraryInspectorPanel.module.css";
import readerStyles from "./ClipDetailDialog.module.css";

export function ClipBodyNotices({ reason, canRetry, onRetry }: {
    reason: string | null;
    canRetry: boolean;
    onRetry: () => void;
}) {
    const { t } = useTranslation();
    return <>
        {reason !== null ? <StateView
            mode={canRetry ? "warning" : "info"}
            placement="inline"
            role="status"
            icon="info"
            title={reason}
            actions={canRetry ? <Button variant="secondary" size="sm" onClick={onRetry}>{t("history.copyAvailability.retry")}</Button> : undefined}
        /> : null}
    </>;
}

export function ClipBodyView({ mode, item, kind, body }: {
    mode: "inspector" | "reader";
    item: Item;
    kind: Kind;
    body: Body;
}) {
    const { t } = useTranslation();
    const reader = mode === "reader";
    const content = body.state === "content" ? body.content : "";
    const unavailable = body.state === "unavailable";
    const loading = body.state === "content" && body.source === "preview";
    if (unavailable || loading) {
        const message = t(unavailable ? "history.detail.fullBodyUnavailable" : "history.empty.loading.title");
        const state = <StateView mode={loading ? "loading" : "error"} placement="panel" role="presentation" title={message} />;
        return reader ? <PreviewSurface role="status" className={readerStyles.unavailable} elevation="flat" border="subtle" radius="md" padding="roomy">{state}</PreviewSurface>
            : <StateView mode={loading ? "loading" : "error"} placement="panel" role="status" title={message} className={inspectorStyles.unavailable} />;
    }
    const preview = <InspectorPreview
        mode={reader ? "reader" : undefined}
        kind={kind}
        ariaLabel={t(reader && kind === "image" ? "history.detail.image" : "history.detail.contents")}
        content={content === "" ? t("history.detail.empty") : content}
        imagePreview={kind === "image" ? <ClipImageLoader
            id={item.id}
            size={reader ? "detail" : "fill"}
            loadingLabel={t("history.detail.imageLoading")}
            failureLabel={t("history.detail.imageUnavailable")}
            title={reader ? t("history.detail.image") : undefined}
        /> : undefined}
    />;
    return reader ? <PreviewSurface
        className={readerStyles.contentRegion}
        elevation="flat"
        border={kind === "code" || kind === "json" ? "none" : "strong"}
        radius="md"
        padding="compact"
    >{preview}</PreviewSurface> : preview;
}
