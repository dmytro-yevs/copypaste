import { useQuery } from "@tanstack/react-query";
import { useId, useMemo, useState } from "react";

import { FieldFeedback } from "@/components/shared";
import { Button, Input, Select, Surface, iconComponent } from "@/components/ui";
import type { SelectItem } from "@/components/ui/select";
import { SourceAppIcon } from "@/features/source-apps";
import { useHistory } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { canonicalExclusion, findExclusion } from "@/lib/exclusions";
import { clipSourceMetadata } from "@/lib/clipSourcePresentation";
import { isAndroidPlatform, isWindowsPlatform } from "@/lib/platform";
import { listInstalledSourceApps, type Item } from "@/lib/ipc";
import {
    POLL_BACKOFF_MS,
    SOURCE_APP_CATALOG_STALE_MS,
} from "@/lib/scheduling";
import styles from "./SourceExclusions.module.css";
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

    const normalized = manualId.trim();
    const installedById = useMemo(
        () =>
            new Map(
                (installedApps.data ?? []).map((app) => [
                    canonicalExclusion(app.package_id, windows) ?? app.package_id,
                    app,
                ]),
            ),
        [installedApps.data, windows],
    );
    const installedOptions = useMemo<SelectItem[]>(
        () => (installedApps.data ?? []).map((app) => ({
            value: canonicalExclusion(app.package_id, windows) ?? app.package_id,
            label: app.label,
            description: app.package_id,
            visual: <SourceAppIcon bundleId={app.package_id} Fallback={iconComponent("app")} />,
        })),
        [installedApps.data, windows],
    );
    const selectedOptions = ids.map((id): SelectItem => {
        const app = installedById.get(id);
        const item = firstByApp.get(id);
        const source = item ? clipSourceMetadata(item) : null;
        return {
            value: id,
            label: app?.label ?? source?.label ?? id,
            description: app?.label || source?.label ? id : undefined,
            visual: <SourceAppIcon
                itemId={item?.id ?? null}
                bundleId={id}
                Fallback={source ? iconComponent(source.icon) : iconComponent("search")}
                size="xs"
            />,
        };
    });

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
            <Select
                mode="multiple"
                display="catalog"
                aria-label={t("settings.service.exclusions.installedList")}
                items={installedOptions}
                values={ids}
                disabled={disabled}
                onValuesChange={(next) => {
                    const added = next.find((id) => !ids.includes(id));
                    if (added !== undefined) add(added);
                    else onChange(next);
                }}
                catalog={{
                    searchLabel: t("settings.service.exclusions.searchInstalled"),
                    listLabel: t("settings.service.exclusions.installedList"),
                    emptyLabel: t("settings.service.exclusions.noInstalledMatches"),
                    loadingLabel: t("settings.service.exclusions.loadingApps"),
                    errorLabel: t("settings.service.exclusions.appsUnavailable"),
                    errorDescription: t("settings.service.exclusions.appsUnavailableBody"),
                    retryLabel: t("settings.service.exclusions.retryApps"),
                    refreshLabel: t("settings.service.exclusions.refreshApps"),
                    removeLabel: (id) => t("settings.service.exclusions.remove", { id }),
                    onRetry: () => void installedApps.refetch(),
                    loading: installedApps.isLoading,
                    refreshing: installedApps.isFetching && !installedApps.isLoading,
                    failed: installedApps.isError,
                    selectedItems: selectedOptions,
                    disableSelectedOptions: true,
                }}
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

        </>
    );
}
