import { useMemo, type RefObject } from "react";

import { Container } from "@/components/layout";
import { ActionButton, InlineNotice, SearchField } from "@/components/shared";
import { MultiSelect, Select, VisuallyHidden } from "@/components/ui";
import { useLibraryToolbarSearch } from "@/features/history/hooks/useLibraryToolbarSearch";
import {
    historyCount,
    historyCountNumber,
    historyDeviceOptions,
    historyKindOptions,
    historySortOptions,
} from "@/features/history/model/libraryToolbarOptions";
import type { OriginDevice } from "@/features/history/model/origin";
import { ActiveControlBadge } from "@/features/history/patterns/ActiveControlBadge";
import { BulkActionBar } from "@/features/history/patterns/BulkActionBar";
import { LibraryToolbarHeader } from "@/features/history/patterns/LibraryToolbarHeader";
import { useTranslation } from "@/i18n";
import {
    DEFAULT_VIEW,
    type SortOrder,
    type ViewOptions,
    kindLabel,
} from "@/lib/view";
import styles from "./LibraryToolbar.module.css";

export { historyCount } from "@/features/history/model/libraryToolbarOptions";

interface ToolbarSelection {
    count: number;
    total: number;
    allSelected: boolean;
    allPinned: boolean;
    busy: boolean;
    onToggleAll: () => void;
    onSelectAll: () => void;
    onTogglePin: () => void;
    onDelete: () => void;
    onClose: () => void;
}

interface LibraryToolbarProps {
    value: string;
    onChange: (value: string) => void;
    onEnterList: () => void;
    inputRef: RefObject<HTMLInputElement | null>;
    filtered: boolean;
    visible: number;
    total: number | undefined;
    view: ViewOptions;
    onViewChange: (view: ViewOptions) => void;
    origins: readonly OriginDevice[];
    displayLimit: number | null;
    selection?: ToolbarSelection;
}

export function LibraryToolbar({
    value,
    onChange,
    onEnterList,
    inputRef,
    filtered,
    visible,
    total,
    view,
    onViewChange,
    origins,
    displayLimit,
    selection,
}: LibraryToolbarProps) {
    const { t } = useTranslation();
    const kindItems = useMemo(historyKindOptions, []);
    const deviceItems = useMemo(
        () => historyDeviceOptions(origins),
        [origins, t],
    );
    const sortItems = useMemo(historySortOptions, []);
    const searching = value.trim().length > 0;
    const kindActive = view.kinds.length > 0;
    const deviceActive = view.devices.length > 0;
    const sortActive = view.sort !== DEFAULT_VIEW.sort;
    const groupActive = view.groupByDevice !== DEFAULT_VIEW.groupByDevice;
    const controlLabel = (control: string, active: boolean) =>
        t(
            active
                ? "history.search.controlActive"
                : "history.search.controlDefault",
            { control },
        );
    const searchLabel = controlLabel(t("history.search.label"), searching);
    const kindLabelText = controlLabel(
        t("history.search.filterKind"),
        kindActive,
    );
    const deviceLabel = controlLabel(
        t("history.search.filterDevice"),
        deviceActive,
    );
    const sortLabelText = controlLabel(
        t("history.search.sortOrder"),
        sortActive,
    );
    const groupLabel = controlLabel(
        t("history.search.groupByDevice"),
        groupActive,
    );
    const {
        closeSearch,
        compactSearch,
        handleSearchKey,
        openSearch,
        overlayRef,
        searchExpanded,
        searchOverlayId,
        searchTriggerId,
        setToolbarRef,
        siblingsRef,
    } = useLibraryToolbarSearch({
        inputRef,
        selectionActive: selection !== undefined,
        value,
        onChange,
        onEnterList,
    });

    return (
        <>
            <LibraryToolbarHeader
                compact={compactSearch}
                filtered={filtered}
                visible={visible}
                total={total}
            />

            <Container
                width="library"
                gutter="screen"
                className={styles.toolbarShell}
            >
                <div
                    ref={setToolbarRef}
                    role="toolbar"
                    aria-label={t(
                        selection
                            ? "history.bulk.label"
                            : "history.search.toolbar",
                    )}
                    data-slot="history-toolbar"
                    data-search-expanded={searchExpanded || undefined}
                    data-compact-search={compactSearch || undefined}
                    className={styles.toolbar}
                >
                    {selection ? (
                        <BulkActionBar {...selection} />
                    ) : (
                        <>
                            <div
                                ref={siblingsRef}
                                className={styles.siblings}
                                aria-hidden={searchExpanded || undefined}
                            >
                                <ActiveControlBadge
                                    active={
                                        searching &&
                                        !compactSearch &&
                                        !searchExpanded
                                    }
                                    className={styles.searchInline}
                                >
                                    <SearchField
                                        size="compact"
                                        inputRef={inputRef}
                                        value={value}
                                        spellCheck={false}
                                        autoComplete="off"
                                        placeholder={t(
                                            "history.search.placeholder",
                                        )}
                                        aria-label={searchLabel}
                                        clearLabel={t("history.search.clear")}
                                        title={t("history.search.hint")}
                                        onChange={(event) =>
                                            onChange(event.target.value)
                                        }
                                        onClear={() => onChange("")}
                                        onKeyDown={handleSearchKey}
                                    />
                                </ActiveControlBadge>
                                <ActiveControlBadge
                                    active={
                                        searching &&
                                        compactSearch &&
                                        !searchExpanded
                                    }
                                    className={styles.searchTriggerFrame}
                                >
                                    <ActionButton
                                        id={searchTriggerId}
                                        className={styles.searchTrigger}
                                        size="compactIcon"
                                        icon="search"
                                        aria-label={searchLabel}
                                        aria-controls={searchOverlayId}
                                        aria-expanded={searchExpanded}
                                        onClick={() => openSearch()}
                                    />
                                </ActiveControlBadge>
                                <ActiveControlBadge active={kindActive}>
                                    <MultiSelect
                                        size="compact"
                                        aria-label={kindLabelText}
                                        values={view.kinds}
                                        items={kindItems}
                                        allLabel={kindLabel("all")}
                                        leadingIcon="sliders"
                                        presentation="auto"
                                        onValuesChange={(kinds) =>
                                            onViewChange({
                                                ...view,
                                                kinds: kinds as ViewOptions["kinds"],
                                            })
                                        }
                                    />
                                </ActiveControlBadge>
                                {origins.length > 1 ? (
                                    <ActiveControlBadge active={deviceActive}>
                                        <MultiSelect
                                            size="compact"
                                            aria-label={deviceLabel}
                                            values={view.devices}
                                            items={deviceItems}
                                            allLabel={t(
                                                "history.search.allDevices",
                                            )}
                                            leadingIcon="devices"
                                            presentation="auto"
                                            onValuesChange={(devices) =>
                                                onViewChange({ ...view, devices })
                                            }
                                        />
                                    </ActiveControlBadge>
                                ) : null}
                                <ActiveControlBadge active={sortActive}>
                                    <Select
                                        size="compact"
                                        aria-label={sortLabelText}
                                        value={view.sort}
                                        items={sortItems}
                                        presentation="auto"
                                        active={sortActive}
                                        onValueChange={(sort) =>
                                            onViewChange({
                                                ...view,
                                                sort: sort as SortOrder,
                                            })
                                        }
                                    />
                                </ActiveControlBadge>
                                <div className={styles.endSlot}>
                                    {origins.length > 1 ? (
                                        <ActiveControlBadge active={groupActive}>
                                            <ActionButton
                                                size="compactIcon"
                                                aria-label={groupLabel}
                                                aria-pressed={
                                                    view.groupByDevice
                                                }
                                                title={groupLabel}
                                                icon="monitor"
                                                onClick={() =>
                                                    onViewChange({
                                                        ...view,
                                                        groupByDevice:
                                                            !view.groupByDevice,
                                                    })
                                                }
                                            />
                                        </ActiveControlBadge>
                                    ) : null}
                                </div>
                                <strong
                                    aria-hidden="true"
                                    className={styles.count}
                                >
                                    <span className={styles.countNumber}>
                                        {historyCountNumber(
                                            filtered,
                                            visible,
                                            total,
                                        )}
                                    </span>
                                    <span className={styles.countLabel}>
                                        {historyCount(filtered, visible, total)}
                                    </span>
                                </strong>
                            </div>

                            {searchExpanded ? (
                                <ActiveControlBadge
                                    id={searchOverlayId}
                                    className={styles.searchOverlay}
                                    active={searching}
                                >
                                    <SearchField
                                        size="compact"
                                        inputRef={overlayRef}
                                        mode="overlay"
                                        expanded
                                        value={value}
                                        spellCheck={false}
                                        autoComplete="off"
                                        placeholder={t(
                                            "history.search.placeholder",
                                        )}
                                        aria-label={searchLabel}
                                        clearLabel={t("history.search.clear")}
                                        closeLabel={t("history.search.close")}
                                        onChange={(event) =>
                                            onChange(event.target.value)
                                        }
                                        onClear={() => onChange("")}
                                        onRequestClose={closeSearch}
                                        onKeyDown={handleSearchKey}
                                    />
                                </ActiveControlBadge>
                            ) : null}
                            <VisuallyHidden aria-live="polite">
                                {historyCount(filtered, visible, total)}
                            </VisuallyHidden>
                        </>
                    )}
                </div>
            </Container>

            {displayLimit !== null ? (
                <InlineNotice live>
                    {t("history.search.displayLimitHint", {
                        limit: displayLimit,
                        count: visible,
                    })}
                </InlineNotice>
            ) : null}
        </>
    );
}
