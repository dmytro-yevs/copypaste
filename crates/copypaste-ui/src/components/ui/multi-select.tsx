import { cn } from "@/lib/cn";
import { ControlAdornment } from "./control-adornment";
import {
    ControlEndSlot,
    controlSurfaceVariants,
    type ControlSurfaceVariants,
} from "./control-surface";
import {
    DropdownMenu,
    DropdownMenuCheckboxItem,
    DropdownMenuContent,
    DropdownMenuTrigger,
} from "./dropdown-menu";
import { Tooltip } from "./tooltip";
import { Icon, type IconName } from "./icon";
import styles from "./multi-select.module.css";

export interface MultiSelectItem {
    readonly value: string;
    readonly label: string;
    readonly icon?: IconName;
}

type MultiSelectProps = ControlSurfaceVariants & {
    values: readonly string[];
    items: readonly MultiSelectItem[];
    allLabel: string;
    onValuesChange: (values: readonly string[]) => void;
    leadingIcon?: IconName;
    presentation?: "auto" | "label" | "icon";
    className?: string;
    "aria-label": string;
};

export function MultiSelect({
    values,
    items,
    allLabel,
    onValuesChange,
    leadingIcon,
    presentation = "label",
    className,
    size = "compact",
    width = "content",
    ...aria
}: MultiSelectProps) {
    const selected = new Set(values);
    const summary =
        values.length === 0
            ? allLabel
            : values.length === 1
              ? (items.find((item) => item.value === values[0])?.label ??
                allLabel)
              : `${values.length} selected`;
    const purpose = aria["aria-label"];
    const accessibleLabel = `${purpose}: ${summary}`;
    const first =
        values.length === 1
            ? items.find((item) => item.value === values[0])
            : undefined;
    const triggerIcon = first?.icon ?? leadingIcon ?? items[0]?.icon;

    const trigger = (
        <DropdownMenuTrigger
            {...aria}
            aria-label={accessibleLabel}
            data-slot="multi-select-trigger"
            data-presentation={presentation}
            data-active-filter={values.length > 0 || undefined}
            className={cn(
                controlSurfaceVariants({ size, width }),
                styles.trigger,
                className,
            )}
        >
            <span className={styles.triggerContents}>
                {triggerIcon ? (
                    <ControlAdornment size="compact" tone="muted">
                        <Icon name={triggerIcon} />
                    </ControlAdornment>
                ) : null}
                <span className={styles.label}>{summary}</span>
            </span>
            <ControlEndSlot>
                <ControlAdornment size="compact" tone="muted">
                    <Icon name="caretDown" weight="bold" />
                </ControlAdornment>
            </ControlEndSlot>
        </DropdownMenuTrigger>
    );

    return (
        <DropdownMenu>
            <Tooltip content={accessibleLabel}>{trigger}</Tooltip>
            <DropdownMenuContent className={styles.content}>
                <DropdownMenuCheckboxItem
                    checked={values.length === 0}
                    onCheckedChange={() => onValuesChange([])}
                    onSelect={(event) => event.preventDefault()}
                >
                    <span className={styles.itemLabel}>{allLabel}</span>
                </DropdownMenuCheckboxItem>
                {items.map((item) => {
                    const checked = selected.has(item.value);
                    return (
                        <DropdownMenuCheckboxItem
                            key={item.value}
                            checked={checked}
                            onCheckedChange={() =>
                                onValuesChange(
                                    checked
                                        ? values.filter(
                                              (value) => value !== item.value,
                                          )
                                        : [...values, item.value],
                                )
                            }
                            onSelect={(event) => event.preventDefault()}
                        >
                            {item.icon ? (
                                <ControlAdornment size="regular" tone="muted">
                                    <Icon name={item.icon} />
                                </ControlAdornment>
                            ) : null}
                            <span className={styles.itemLabel}>
                                {item.label}
                            </span>
                        </DropdownMenuCheckboxItem>
                    );
                })}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
