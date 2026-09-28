import * as SelectPrimitive from "@radix-ui/react-select";
import { useDeferredValue, useId, useMemo, useRef, useState, type ReactNode, type KeyboardEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { ControlEndSlot, controlSurfaceVariants, type ControlSurfaceVariants } from "./control-surface";
import { ControlAdornment } from "./control-adornment";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuTrigger } from "./dropdown-menu";
import { Tooltip } from "./tooltip";
import { Icon, type IconName } from "./icon";
import { Input } from "./input";
import { Button } from "./button";
import { StateView } from "@/components/shared/StateView";
import { cn } from "@/lib/cn";
import styles from "./select.module.css";

/** Metadata is supplied by the caller; Select never owns feature identities. */
export interface SelectItem {
    readonly value: string;
    readonly label: string;
    readonly icon?: IconName;
    readonly visual?: ReactNode;
    readonly description?: string;
}

type SharedProps = ControlSurfaceVariants & {
    items: readonly SelectItem[];
    leadingIcon?: IconName;
    presentation?: "auto" | "label" | "icon";
    className?: string;
    disabled?: boolean;
    id?: string;
    "aria-label"?: string;
    "aria-labelledby"?: string;
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    "aria-errormessage"?: string;
    "aria-busy"?: boolean;
};

type SingleProps = SharedProps & {
    mode?: "single";
    value: string;
    onValueChange: (value: string) => void;
    measure?: "auto" | "regular" | "wide";
    active?: boolean;
    placeholder?: string;
    values?: never;
    onValuesChange?: never;
};

export interface SelectCatalog {
    searchLabel: string;
    listLabel: string;
    emptyLabel: string;
    loadingLabel: string;
    errorLabel: string;
    errorDescription?: string;
    retryLabel: string;
    refreshLabel: string;
    removeLabel: (value: string) => string;
    onRetry: () => void;
    loading?: boolean;
    refreshing?: boolean;
    failed?: boolean;
    /** Keep selected entries visible independently of this filtered catalogue. */
    selectedItems?: readonly SelectItem[];
    /** A catalog can be refreshed while its selected values remain available. */
    disableSelectedOptions?: boolean;
};

type MultipleBase = SharedProps & {
    mode: "multiple";
    values: readonly string[];
    onValuesChange: (values: string[]) => void;
    value?: never;
    onValueChange?: never;
};
type DropdownMultipleProps = MultipleBase & { display?: "dropdown"; allLabel: string; catalog?: never };
type CatalogMultipleProps = MultipleBase & { display: "catalog"; catalog: SelectCatalog; allLabel?: never };
type MultipleProps = DropdownMultipleProps | CatalogMultipleProps;

export type SelectProps = SingleProps | MultipleProps;

function triggerAccessibility(props: SharedProps) {
    return {
        id: props.id,
        "aria-labelledby": props["aria-labelledby"],
        "aria-describedby": props["aria-describedby"],
        "aria-invalid": props["aria-invalid"],
        "aria-errormessage": props["aria-errormessage"],
        "aria-busy": props["aria-busy"],
    };
}

function OptionContent({ item }: { item: SelectItem }) {
    return (
        <>
            {item.visual ?? (item.icon ? (
                <ControlAdornment size="regular" tone="muted">
                    <Icon name={item.icon} />
                </ControlAdornment>
            ) : null)}
            <span className={styles.optionCopy}>
                <span className={styles.itemLabel} title={item.label}>{item.label}</span>
                {item.description && <span className={styles.itemDescription} title={item.description}>{item.description}</span>}
            </span>
        </>
    );
}

function SelectTrigger({
    summary, icon, presentation, className, size, width, state, disabled, measure,
    active, slot, kind, ...aria
}: {
    summary: string;
    icon?: IconName;
    presentation: "auto" | "label" | "icon";
    className?: string;
    size?: ControlSurfaceVariants["size"];
    width?: ControlSurfaceVariants["width"];
    state?: ControlSurfaceVariants["state"];
    disabled?: boolean;
    measure?: "auto" | "regular" | "wide";
    active?: boolean;
    slot: string;
    kind: "single" | "multiple";
} & Pick<SharedProps, "id" | "aria-label" | "aria-labelledby" | "aria-describedby" | "aria-invalid" | "aria-errormessage" | "aria-busy">) {
    const adornmentSize = size === "compact" || size === "sm" ? "compact" : "regular";
    return (
        <Tooltip content={summary}>
            <span className={styles.tooltipAnchor}>
                {kind === "single" ? (
                    <SelectPrimitive.Trigger
                        {...aria}
                        disabled={disabled}
                        data-slot={slot}
                        data-presentation={presentation}
                        data-active-filter={active || undefined}
                        className={cn(controlSurfaceVariants({ size: size ?? "md", width: width ?? "content", state: disabled ? "disabled" : state }), styles.trigger, width === "fill" ? undefined : styles[measure ?? "auto"], className)}
                    >
                        <span className={styles.triggerLayout}><span className={styles.triggerContents}>
                            {icon && <ControlAdornment size={adornmentSize} tone="muted"><Icon name={icon} /></ControlAdornment>}
                            <span className={styles.label}>{summary}</span>
                        </span><ControlEndSlot><ControlAdornment size={adornmentSize} tone="muted"><SelectPrimitive.Icon asChild><Icon name="caretDown" weight="bold" className={styles.caret} /></SelectPrimitive.Icon></ControlAdornment></ControlEndSlot></span>
                    </SelectPrimitive.Trigger>
                ) : (
                    <DropdownMenuTrigger
                        {...aria}
                        disabled={disabled}
                        data-slot={slot}
                        data-presentation={presentation}
                        data-active-filter={active || undefined}
                        className={cn(controlSurfaceVariants({ size: size ?? "compact", width: width ?? "content", state: disabled ? "disabled" : state }), styles.trigger, className)}
                    >
                        <span className={styles.triggerLayout}><span className={styles.triggerContents}>
                            {icon && <ControlAdornment size={adornmentSize} tone="muted"><Icon name={icon} /></ControlAdornment>}
                            <span className={styles.label}>{summary}</span>
                        </span><ControlEndSlot><ControlAdornment size={adornmentSize} tone="muted"><Icon name="caretDown" weight="bold" /></ControlAdornment></ControlEndSlot></span>
                    </DropdownMenuTrigger>
                )}
            </span>
        </Tooltip>
    );
}

function SingleSelect(props: SingleProps) {
    const { value, items, onValueChange, leadingIcon, measure, presentation = "label", className, active, placeholder, disabled, size, width, state } = props;
    const selected = items.find((item) => item.value === value);
    const summary = selected?.label ?? placeholder ?? value;
    const purpose = props["aria-label"] ?? "Select";
    const accessibleLabel = `${purpose}: ${summary}`;
    return (
        <SelectPrimitive.Root value={value} onValueChange={onValueChange} disabled={disabled}>
            <SelectTrigger {...triggerAccessibility(props)} aria-label={accessibleLabel} summary={summary} icon={leadingIcon ?? selected?.icon} presentation={presentation} className={className} active={active} disabled={disabled} size={size} width={width} state={state} measure={measure} slot="select-trigger" kind="single" />
            <SelectPrimitive.Portal>
                <SelectPrimitive.Content position="popper" sideOffset={8} collisionPadding={8} className={styles.content}>
                    <SelectPrimitive.Viewport className={styles.viewport}>
                        {items.map((item) => (
                            <SelectPrimitive.Item key={item.value} value={item.value} data-value={item.value} className={styles.item}>
                                <SelectPrimitive.ItemText asChild><span className={styles.optionContainer}><OptionContent item={item} /></span></SelectPrimitive.ItemText>
                                <SelectPrimitive.ItemIndicator className={styles.indicator}><Icon name="check" weight="bold" className={styles.check} /></SelectPrimitive.ItemIndicator>
                            </SelectPrimitive.Item>
                        ))}
                    </SelectPrimitive.Viewport>
                </SelectPrimitive.Content>
            </SelectPrimitive.Portal>
        </SelectPrimitive.Root>
    );
}

function CatalogSelect({ items, values, onValuesChange, catalog, disabled }: CatalogMultipleProps) {
    const [query, setQuery] = useState("");
    const deferredQuery = useDeferredValue(query);
    const searchId = useId();
    const scrollRef = useRef<HTMLDivElement>(null);
    const selected = useMemo(() => new Set(values), [values]);
    const allItems = useMemo(() => new Map([...items, ...(catalog.selectedItems ?? [])].map((item) => [item.value, item])), [items, catalog.selectedItems]);
    const selectedItems = values.map((value) => allItems.get(value) ?? { value, label: value });
    const visible = useMemo(() => {
        const needle = deferredQuery.trim().toLocaleLowerCase();
        return needle ? items.filter((item) => item.label.toLocaleLowerCase().includes(needle) || item.value.toLocaleLowerCase().includes(needle) || item.description?.toLocaleLowerCase().includes(needle)) : items;
    }, [items, deferredQuery]);
    const virtualizer = useVirtualizer({ count: visible.length, getScrollElement: () => scrollRef.current, estimateSize: () => 52, getItemKey: (index) => visible[index]?.value ?? index, overscan: 8, useFlushSync: false });
    const choose = (value: string) => onValuesChange(selected.has(value) ? values.filter((candidate) => candidate !== value) : [...values, value]);
    const onListKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
        if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
        const index = Number((event.target as HTMLElement).closest<HTMLElement>("[data-index]")?.dataset.index ?? -1);
        if (index < 0) return;
        event.preventDefault();
        const next = Math.max(0, Math.min(visible.length - 1, index + (event.key === "ArrowDown" ? 1 : -1)));
        virtualizer.scrollToIndex(next);
        requestAnimationFrame(() => scrollRef.current?.querySelector<HTMLElement>(`[data-index="${next}"] button`)?.focus());
    };
    return (
        <div className={styles.catalog}>
            <div className={styles.catalogToolbar}>
                <label className={styles.visuallyHidden} htmlFor={searchId}>{catalog.searchLabel}</label>
                <Input id={searchId} size="sm" value={query} disabled={disabled} aria-label={catalog.searchLabel} placeholder={catalog.searchLabel} onChange={(event) => setQuery(event.target.value)} />
                <Button type="button" variant="ghost" size="compactIcon" disabled={disabled || catalog.refreshing} aria-label={catalog.refreshLabel} onClick={catalog.onRetry}><Icon name="refresh" aria-hidden="true" /></Button>
            </div>
            <div ref={scrollRef} className={styles.catalogList} aria-busy={catalog.loading || catalog.refreshing || undefined} onKeyDown={onListKeyDown}>
                {catalog.loading ? <StateView mode="loading" placement="panel" className={styles.catalogState} title={catalog.loadingLabel} />
                : catalog.failed ? <StateView mode="error" placement="panel" className={styles.catalogState} title={catalog.errorLabel} description={catalog.errorDescription} actions={<Button type="button" variant="secondary" size="sm" disabled={disabled || catalog.refreshing} onClick={catalog.onRetry}>{catalog.retryLabel}</Button>} />
                : visible.length === 0 ? <StateView mode="empty" placement="panel" className={styles.catalogState} title={catalog.emptyLabel} />
                : <div role="list" aria-label={catalog.listLabel} className={styles.virtualList} style={{ height: virtualizer.getTotalSize() }}>
                    {virtualizer.getVirtualItems().map((row) => {
                        const item = visible[row.index];
                        if (!item) return null;
                        const checked = selected.has(item.value);
                        return <div key={row.key} ref={virtualizer.measureElement} data-index={row.index} role="listitem" className={styles.catalogRow} style={{ transform: `translateY(${row.start}px)` }}>
                            <button type="button" className={styles.catalogOption} aria-pressed={checked} disabled={disabled || (checked && catalog.disableSelectedOptions)} onClick={() => choose(item.value)}><OptionContent item={item} /><Icon name={checked ? "check" : "plus"} size="sm" className={styles.optionAction} aria-hidden="true" /></button>
                        </div>;
                    })}
                </div>}
            </div>
            {selectedItems.length > 0 && <ul className={styles.selectedList}>{selectedItems.map((item) => <li key={item.value} className={styles.selectedItem}><OptionContent item={item} /><button type="button" className={styles.removeButton} disabled={disabled} aria-label={catalog.removeLabel(item.value)} onClick={() => choose(item.value)}><Icon name="trash" size="sm" aria-hidden="true" /></button></li>)}</ul>}
        </div>
    );
}

function MultipleSelect(props: MultipleProps) {
    if (props.display === "catalog") return <CatalogSelect {...props} />;
    const { values, items, onValuesChange, allLabel, leadingIcon, presentation = "label", className, disabled, size, width, state } = props;
    const selected = new Set(values);
    const first = values.length === 1 ? items.find((item) => item.value === values[0]) : undefined;
    const summary = values.length === 0 ? allLabel : values.length === 1 ? first?.label ?? values[0] : `${values.length} selected`;
    const accessibleLabel = `${props["aria-label"] ?? "Select"}: ${summary}`;
    return <DropdownMenu>
        <SelectTrigger {...triggerAccessibility(props)} aria-label={accessibleLabel} summary={summary} icon={first?.icon ?? leadingIcon ?? items[0]?.icon} presentation={presentation} className={className} active={values.length > 0} disabled={disabled} size={size} width={width} state={state} slot="select-trigger" kind="multiple" />
        <DropdownMenuContent className={styles.multiContent}>
            <DropdownMenuCheckboxItem checked={values.length === 0} onCheckedChange={() => onValuesChange([])} onSelect={(event) => event.preventDefault()}>{allLabel}</DropdownMenuCheckboxItem>
            {items.map((item) => <DropdownMenuCheckboxItem key={item.value} checked={selected.has(item.value)} onCheckedChange={() => onValuesChange(selected.has(item.value) ? values.filter((value) => value !== item.value) : [...values, item.value])} onSelect={(event) => event.preventDefault()}><OptionContent item={item} /></DropdownMenuCheckboxItem>)}
        </DropdownMenuContent>
    </DropdownMenu>;
}

/** One public control with single and multiple selection modes. */
export function Select(props: SelectProps) {
    return props.mode === "multiple" ? <MultipleSelect {...props} /> : <SingleSelect {...props} />;
}
