import * as SelectPrimitive from "@radix-ui/react-select";

import {
    ControlEndSlot,
    controlSurfaceVariants,
    type ControlSurfaceVariants,
} from "./control-surface";
import { ControlAdornment } from "./control-adornment";
import { Tooltip } from "./tooltip";
import { Icon, type IconName } from "./icon";
import { cn } from "@/lib/cn";
import styles from "./select.module.css";

export interface SelectItem {
    readonly value: string;
    readonly label: string;
    readonly icon?: IconName;
}

type SelectProps = ControlSurfaceVariants & {
    value: string;
    items: readonly SelectItem[];
    onValueChange: (value: string) => void;
    leadingIcon?: IconName;
    measure?: "auto" | "regular" | "wide";
    presentation?: "auto" | "label" | "icon";
    className?: string;
    active?: boolean;
    placeholder?: string;
    disabled?: boolean;
    id?: string;
    "aria-label"?: string;
    "aria-labelledby"?: string;
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    "aria-errormessage"?: string;
};

export function Select({
    value,
    items,
    onValueChange,
    leadingIcon,
    measure = "auto",
    presentation = "label",
    className,
    active = false,
    placeholder,
    disabled = false,
    size,
    width = "content",
    state,
    ...aria
}: SelectProps) {
    const selected =
        items.find((item) => item.value === value) ??
        (placeholder ? { value: "", label: placeholder } : items[0]);
    const selectedIcon = leadingIcon ?? selected?.icon;
    const adornmentSize =
        size === "compact" || size === "sm" ? "compact" : "regular";
    const purpose = aria["aria-label"] ?? "Select";
    const accessibleLabel = `${purpose}: ${selected?.label ?? placeholder ?? value}`;
    const trigger = (
        <SelectPrimitive.Trigger
            {...aria}
            aria-label={accessibleLabel}
            data-slot="select-trigger"
            data-value={value}
            data-presentation={presentation}
            data-active-filter={active || undefined}
            className={cn(
                controlSurfaceVariants({
                    size: size ?? "md",
                    width,
                    state: disabled ? "disabled" : state,
                }),
                styles.trigger,
                width === "fill" ? undefined : styles[measure],
                className,
            )}
        >
            <span className={styles.triggerLayout}>
                <span className={styles.triggerContents}>
                    {selectedIcon ? (
                        <ControlAdornment size={adornmentSize} tone="muted">
                            <Icon name={selectedIcon} />
                        </ControlAdornment>
                    ) : null}
                    <span className={styles.label}>{selected?.label}</span>
                </span>
                <ControlEndSlot>
                    <ControlAdornment size={adornmentSize} tone="muted">
                        <SelectPrimitive.Icon asChild>
                            <Icon
                                name="caretDown"
                                weight="bold"
                                className={styles.caret}
                            />
                        </SelectPrimitive.Icon>
                    </ControlAdornment>
                </ControlEndSlot>
            </span>
        </SelectPrimitive.Trigger>
    );

    return (
        <SelectPrimitive.Root
            value={value}
            onValueChange={onValueChange}
            disabled={disabled}
        >
            <Tooltip content={accessibleLabel}>
                <span className={styles.tooltipAnchor}>{trigger}</span>
            </Tooltip>
            <SelectPrimitive.Portal>
                <SelectPrimitive.Content
                    position="popper"
                    sideOffset={8}
                    collisionPadding={8}
                    className={styles.content}
                >
                    <SelectPrimitive.Viewport className={styles.viewport}>
                        {items.map((item) => {
                            return (
                                <SelectPrimitive.Item
                                    key={item.value}
                                    value={item.value}
                                    data-value={item.value}
                                    className={styles.item}
                                >
                                    {item.icon ? (
                                        <ControlAdornment
                                            size="regular"
                                            tone="muted"
                                        >
                                            <Icon name={item.icon} />
                                        </ControlAdornment>
                                    ) : null}
                                    <SelectPrimitive.ItemText asChild>
                                        <span className={styles.itemLabel}>
                                            {item.label}
                                        </span>
                                    </SelectPrimitive.ItemText>
                                    <SelectPrimitive.ItemIndicator
                                        className={styles.indicator}
                                    >
                                        <Icon
                                            name="check"
                                            weight="bold"
                                            className={styles.check}
                                        />
                                    </SelectPrimitive.ItemIndicator>
                                </SelectPrimitive.Item>
                            );
                        })}
                    </SelectPrimitive.Viewport>
                </SelectPrimitive.Content>
            </SelectPrimitive.Portal>
        </SelectPrimitive.Root>
    );
}
