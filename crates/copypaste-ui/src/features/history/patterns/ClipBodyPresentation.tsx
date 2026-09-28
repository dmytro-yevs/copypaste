import { Button, Icon } from "@/components/ui";
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

export function ClipBodyNotices({ reason, canRetry, onRetry, potentialFinding }: {
    reason: string | null;
    canRetry: boolean;
    onRetry: () => void;
    potentialFinding: boolean;
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
        {potentialFinding ? <StateView mode="warning" placement="inline" role="status" icon="sensitive" title={t("history.row.potentialSensitiveWarning")} /> : null}
    </>;
}

export function ClipPotentialRevealButton({ revealed, disabled, onClick }: {
    revealed: boolean;
    disabled: boolean;
    onClick: () => void;
}) {
    const { t } = useTranslation();
    return <Button variant="secondary" icon={revealed ? "eyeOff" : "eye"} disabled={disabled} aria-pressed={revealed} onClick={onClick}>
        {t(revealed ? "history.row.hideOriginal" : "history.row.showOriginal")}
    </Button>;
}

export function ClipBodyView({ mode, item, kind, body, copyPending, revealPending, onReveal }: {
    mode: "inspector" | "reader";
    item: Item;
    kind: Kind;
    body: Body;
    copyPending: boolean;
    revealPending: boolean;
    onReveal: (item: Item) => void;
}) {
    const { t } = useTranslation();
    const reader = mode === "reader";
    const content = body.state === "content" ? body.content : "";
    if (body.state === "masked") {
        return <Button
            type="button"
            variant="ghost"
            disabled={copyPending}
            aria-label={t("history.row.sensitiveReveal")}
            aria-busy={revealPending || undefined}
            className={reader ? readerStyles.masked : inspectorStyles.protected}
            onClick={() => !revealPending && onReveal(item)}
        >
            {reader ? <>
                <span aria-hidden="true" className={readerStyles.redactions}>
                    <span className={readerStyles.redactionLong} />
                    <span className={readerStyles.redactionShort} />
                    <span className={readerStyles.redactionMedium} />
                </span>
                {revealPending && <Icon name="spinner" className={readerStyles.spinner} />}
            </> : <>
                <span aria-hidden="true" className={inspectorStyles.protectedLines}><i /><i /><i /></span>
                <Icon name="eye" size="sm" />
                <strong>{t("history.row.sensitivePlaceholder")}</strong>
            </>}
        </Button>;
    }
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
