import { useQuery } from "@tanstack/react-query";
import { useDeferredValue, useId, useMemo, useState } from "react";

import { FieldFeedback } from "@/components/shared";
import { Button, Input, Surface } from "@/components/ui";
import { useHistory } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { canonicalExclusion, findExclusion } from "@/lib/exclusions";
import { isAndroidPlatform, isWindowsPlatform } from "@/lib/platform";
import { listInstalledSourceApps, type Item } from "@/lib/ipc";
import {
    POLL_BACKOFF_MS,
    SOURCE_APP_CATALOG_STALE_MS,
} from "@/lib/scheduling";
import styles from "./SourceExclusions.module.css";
import { InstalledAppPicker } from "./InstalledAppPicker";
import { SelectedExclusions } from "./SelectedExclusions";
import { SourceExclusionsHeader } from "./SourceExclusionsHeader";

interface SourceExclusionsProps {
    ids: readonly string[];
    disabled?: boolean;
    collapsible?: boolean;
    onChange: (ids: string[]) => void;
}

/** The service is the source of truth. Native catalogues provide display
 * metadata; only the platform identity is persisted. */
export function SourceExclusions({
    ids,
    disabled = false,
    collapsible = false,
    onChange,
}: SourceExclusionsProps) {
    const { t } = useTranslation();
    const [expanded, setExpanded] = useState(!collapsible);
    const controlsId = useId();
    const android = isAndroidPlatform();
    const windows = isWindowsPlatform();

    // The region the button controls stays mounted and is hidden, because INV-7
    // forbids an accessibility pointer to a node that is not there. The *editor*
    // is unmounted: it owns the history query, and mounting it starts a 3s poll
    // that decrypts a page of clipboard history for a panel nobody has opened.
    return (
        <Surface asChild elevation="raised" border="subtle" radius="md">
            <section
                data-settings-search-target={`section:${t("settings.service.exclusions.title")}`}
                className={styles.root}
            >
                <SourceExclusionsHeader
                    collapsible={collapsible}
                    expanded={expanded}
                    controlsId={controlsId}
                    android={android}
                    windows={windows}
                    onToggle={() => setExpanded((current) => !current)}
                />
                <div
                    id={controlsId}
                    hidden={!expanded}
                    className={styles.editorRegion}
                >
                    {expanded && (
                        <ExclusionsEditor
                            ids={ids}
                            disabled={disabled}
                            windows={windows}
                            onChange={onChange}
                        />
                    )}
                </div>
            </section>
        </Surface>
    );
}

interface ExclusionsEditorProps {
    ids: readonly string[];
    disabled: boolean;
    windows: boolean;
    onChange: (ids: string[]) => void;
}

function ExclusionsEditor({
    ids,
    disabled,
    windows,
    onChange,
}: ExclusionsEditorProps) {
    const { t } = useTranslation();
    const history = useHistory("");
    const [catalogQuery, setCatalogQuery] = useState("");
    const deferredCatalogQuery = useDeferredValue(catalogQuery);
    const [manualId, setManualId] = useState("");
    const [validation, setValidation] = useState<string | null>(null);
    const [normalizedNotice, setNormalizedNotice] = useState<string | null>(
        null,
    );
    const validationId = useId();
    const noticeId = useId();

    const installedApps = useQuery({
        queryKey: ["installed-source-apps"],
        queryFn: listInstalledSourceApps,
        staleTime: SOURCE_APP_CATALOG_STALE_MS,
        refetchInterval: (query) =>
            query.state.status === "error" ? POLL_BACKOFF_MS : false,
    });

    /** One pass over the history, not one per proposed id: the `find` this
     *  replaces ran inside the render loop, so a 10,000-row history times the
     *  distinct apps in it was scanned on every keystroke. */
    const firstByApp = useMemo(() => {
        const first = new Map<string, Item>();
        for (const item of history.data?.items ?? []) {
            const app = item.source_app_bundle_id;
            if (app !== null && !first.has(app)) first.set(app, item);
        }
        return first;
    }, [history.data?.items]);

    const selectedIds = useMemo(() => new Set(ids), [ids]);
    const normalized = manualId.trim();
    const deferredNormalized = deferredCatalogQuery.trim();
    const visibleInstalledApps = useMemo(() => {
        const needle = deferredNormalized.toLocaleLowerCase();
        return (installedApps.data ?? []).filter(
            (app) =>
                !needle ||
                app.label.toLocaleLowerCase().includes(needle) ||
                app.package_id.toLocaleLowerCase().includes(needle),
        );
    }, [deferredNormalized, installedApps.data]);
    const installedById = useMemo(
        () =>
            new Map(
                (installedApps.data ?? []).map((app) => [app.package_id, app]),
            ),
        [installedApps.data],
    );

    /** Windows: Chrome.exe, chrome, and a pasted path are one program. */
    const add = (id: string) => {
        setNormalizedNotice(null);
        const next = canonicalExclusion(id, windows);
        if (next === null) {
            setValidation(
                t(
                    windows
                        ? "settings.service.exclusions.windowsInvalid"
                        : "settings.service.exclusions.invalid",
                ),
            );
            return;
        }
        const existing = findExclusion(ids, next, windows);
        if (existing !== undefined) {
            setValidation(
                windows
                    ? t("settings.service.exclusions.windowsExists", {
                          id: existing,
                      })
                    : t("settings.service.exclusions.exists"),
            );
            return;
        }
        setValidation(null);
        setManualId("");
        if (next !== id.trim()) {
            setNormalizedNotice(
                t("settings.service.exclusions.normalized", { id: next }),
            );
        }
        onChange([...ids, next]);
    };

    return (
        <>
            <InstalledAppPicker
                apps={visibleInstalledApps}
                selectedIds={selectedIds}
                query={catalogQuery}
                disabled={disabled}
                loading={installedApps.isLoading}
                refreshing={
                    installedApps.isFetching && !installedApps.isLoading
                }
                failed={installedApps.isError}
                onQueryChange={setCatalogQuery}
                onRetry={() => void installedApps.refetch()}
                onAdd={add}
            />

            <div className={styles.manualEntry}>
                <div className={styles.manualCopy}>
                    <p className={styles.manualTitle}>
                        {t("settings.service.exclusions.manualTitle")}
                    </p>
                    <p className={styles.description}>
                        {t("settings.service.exclusions.manualDescription")}
                    </p>
                </div>
                <div className={styles.entryForm}>
                    <Input
                        size="sm"
                        width="fill"
                        state={
                            disabled
                                ? "disabled"
                                : validation
                                  ? "invalid"
                                  : "normal"
                        }
                        className={styles.entryInput}
                        aria-label={t(
                            windows
                                ? "settings.service.exclusions.windowsInputLabel"
                                : "settings.service.exclusions.inputLabel",
                        )}
                        value={manualId}
                        disabled={disabled}
                        placeholder={t(
                            windows
                                ? "settings.service.exclusions.windowsPlaceholder"
                                : "settings.service.exclusions.placeholder",
                        )}
                        aria-invalid={validation !== null || undefined}
                        aria-describedby={
                            validation
                                ? validationId
                                : normalizedNotice
                                  ? noticeId
                                  : undefined
                        }
                        onChange={(event) => {
                            setManualId(event.target.value);
                            setValidation(null);
                            setNormalizedNotice(null);
                        }}
                        onKeyDown={(event) => {
                            if (event.key !== "Enter") return;
                            event.preventDefault();
                            add(normalized);
                        }}
                    />
                    <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        disabled={disabled || normalized.length === 0}
                        onClick={() => add(normalized)}
                    >
                        {t("settings.service.exclusions.add")}
                    </Button>
                </div>
                {validation && (
                    <FieldFeedback id={validationId} state="error">
                        {validation}
                    </FieldFeedback>
                )}
                {!validation && normalizedNotice && (
                    <FieldFeedback id={noticeId} state="neutral" announce>
                        {normalizedNotice}
                    </FieldFeedback>
                )}
            </div>

            <SelectedExclusions
                ids={ids}
                installedById={installedById}
                firstByApp={firstByApp}
                disabled={disabled}
                onChange={onChange}
            />
        </>
    );
}
