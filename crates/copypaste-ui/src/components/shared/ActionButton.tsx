import type { ComponentProps, ReactNode } from "react";

import { Button, ControlAdornment, Icon, type IconName } from "@/components/ui";
import { cn } from "@/lib/cn";
import { IconButton } from "./IconButton";
import styles from "./ActionButton.module.css";

type ActionSize = "compact" | "compactIcon" | "sm" | "md" | "lg" | "icon";

export interface ActionButtonProps extends Omit<
    ComponentProps<typeof Button>,
    "size" | "tone" | "children"
> {
    size?: ActionSize;
    tone?: "neutral" | "danger";
    edge?: "none" | "control";
    icon?: IconName;
    children?: ReactNode;
}

export function ActionButton({
    variant,
    tone,
    size = "md",
    edge = "none",
    className,
    icon,
    children,
    disabled,
    type = "button",
    ...props
}: ActionButtonProps) {
    const iconOnly = size === "icon" || size === "compactIcon";
    const adornmentSize =
        size === "compact" || size === "compactIcon" ? "compact" : "regular";
    const label = props["aria-label"] ?? props.title;
    if (iconOnly && label && icon) {
        return (
            <IconButton
                {...props}
                type={type}
                disabled={disabled}
                variant={variant ?? "secondary"}
                tone={tone}
                size={size === "compactIcon" ? "compact" : "regular"}
                edge={edge}
                label={label}
                className={className}
                icon={icon}
            />
        );
    }

    return (
        <Button
            {...props}
            type={type}
            disabled={disabled}
            variant={variant ?? "secondary"}
            tone={tone}
            size={size}
            data-action-size={iconOnly ? "icon" : "label"}
            className={cn(className, edge === "control" && styles.controlEdge)}
        >
            {icon ? (
                <ControlAdornment size={adornmentSize}>
                    <Icon
                        name={icon}
                        size={adornmentSize === "compact" ? "sm" : "md"}
                    />
                </ControlAdornment>
            ) : null}
            {children}
        </Button>
    );
}
