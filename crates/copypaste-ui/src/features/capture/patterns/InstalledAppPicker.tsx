import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { Stack } from "@/components/layout";
import { ActionButton, InlineNotice } from "@/components/shared";
import { Icon } from "@/components/ui/icon";
import { SourceAppIcon } from "@/features/source-apps";
import { Button, Input, VisuallyHidden, iconComponent } from "@/components/ui";
import { useTranslation } from "@/i18n";
import type { InstalledSourceApp } from "@/lib/ipc";

import styles from "./SourceExclusions.module.css";

const CATALOG_ROW_ESTIMATE_PX = 52;
const CATALOG_PLACEHOLDER_ROWS = 4;

export interface InstalledAppPickerProps {
    apps: readonly InstalledSourceApp[];
    selectedIds: ReadonlySet<string>;
    query: string;
    disabled: boolean;
    loading: boolean;
    refreshing: boolean;
    failed: boolean;
    onQueryChange: (value: string) => void;
    onRetry: () => void;
    onAdd: (id: string) => void;
}

export function InstalledAppPicker({
    apps,
    selectedIds,
    query,
    disabled,
    loading,
    refreshing,
    failed,
    onQueryChange,
    onRetry,
    onAdd,
}: InstalledAppPickerProps) {
    const { t } = useTranslation();
    const searchLabel = t("settings.service.exclusions.searchInstalled");
    const scrollRef = useRef<HTMLDivElement>(null);
    const virtualizer = useVirtualizer({
        count: apps.length,
        getScrollElement: () => scrollRef.current,
        estimateSize: () => CATALOG_ROW_ESTIMATE_PX,
        getItemKey: (index) => apps[index]?.package_id ?? index,
        overscan: 8,
        useFlushSync: false,
    });

    return (
        <div className={styles.installedPicker}>
            <div className={styles.catalogToolbar}>
                <VisuallyHidden asChild>
                    <label htmlFor="source-exclusion-search">{searchLabel}</label>
                </VisuallyHidden>
                <Input
                    size="sm"
                    id="source-exclusion-search"
                    aria-label={searchLabel}
                    value={query}
                    disabled={disabled}
                    placeholder={searchLabel}
                    onChange={(event) => onQueryChange(event.target.value)}
                />
                <ActionButton
                    type="button"
                    variant="ghost"
                    size="compactIcon"
                    icon="refresh"
                    aria-label={t("settings.service.exclusions.refreshApps")}
                    disabled={disabled || refreshing}
                    onClick={onRetry}
                />
            </div>
            <div
                ref={scrollRef}
                className={styles.installedList}
                aria-busy={loading || refreshing || undefined}
            >
                {loading ? (
                    <div role="status" className={styles.catalogLoading}>
                        <span className={styles.visuallyNamedState}>
                            {t("settings.service.exclusions.loadingApps")}
                        </span>
                        <div aria-hidden="true">
                            {Array.from({ length: CATALOG_PLACEHOLDER_ROWS }).map((_, index) => (
                                <div key={index} className={styles.catalogSkeletonRow}>
                                    <span className={styles.catalogSkeletonIcon} />
                                    <span className={styles.catalogSkeletonCopy}>
                                        <span className={styles.catalogSkeletonLabel} />
                                        <span className={styles.catalogSkeletonId} />
                                    </span>
                                </div>
                            ))}
                        </div>
                    </div>
                ) : failed ? (
                    <div className={styles.catalogNotice}>
                        <InlineNotice
                            role="alert"
                            tone="danger"
                            icon="alert"
                            action={
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    disabled={disabled || refreshing}
                                    onClick={onRetry}
                                >
                                    <Icon name="refresh" size="sm" aria-hidden="true" />
                                    {t("settings.service.exclusions.retryApps")}
                                </Button>
                            }
                        >
                            <Stack asChild gap="xs">
                                <span>
                                    <strong>
                                        {t("settings.service.exclusions.appsUnavailable")}
                                    </strong>
                                    <small>
                                        {t("settings.service.exclusions.appsUnavailableBody")}
                                    </small>
                                </span>
                            </Stack>
                        </InlineNotice>
                    </div>
                ) : apps.length === 0 ? (
                    <div role="status" className={styles.catalogStateCopy}>
                        <Icon name="searchX" size="md" aria-hidden="true" />
                        <span>{t("settings.service.exclusions.noInstalledMatches")}</span>
                    </div>
                ) : (
                    <div
                        role="list"
                        aria-label={t("settings.service.exclusions.installedList")}
                        className={styles.virtualList}
                        style={{ height: virtualizer.getTotalSize() }}
                    >
                        {virtualizer.getVirtualItems().map((virtualRow) => {
                            const app = apps[virtualRow.index];
                            if (!app) return null;
                            const selected = selectedIds.has(app.package_id);
                            return (
                                <div
                                    key={virtualRow.key}
                                    ref={virtualizer.measureElement}
                                    data-index={virtualRow.index}
                                    role="listitem"
                                    className={styles.installedRow}
                                    style={{
                                        transform: `translateY(${virtualRow.start}px)`,
                                    }}
                                >
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="sm"
                                        disabled={disabled || selected}
                                        className={styles.installedApp}
                                        onClick={() => onAdd(app.package_id)}
                                    >
                                        <SourceAppIcon
                                            bundleId={app.package_id}
                                            Fallback={iconComponent("app")}
                                        />
                                        <span className={styles.appCopy}>
                                            <span className={styles.appLabel} title={app.label}>
                                                {app.label}
                                            </span>
                                            <span className={styles.appId} title={app.package_id}>
                                                {app.package_id}
                                            </span>
                                        </span>
                                        <Icon
                                            name={selected ? "check" : "plus"}
                                            size="sm"
                                            className={styles.addIcon}
                                            aria-hidden="true"
                                        />
                                    </Button>
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>
        </div>
    );
}
