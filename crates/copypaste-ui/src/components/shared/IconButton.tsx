import type { ComponentProps } from "react";

import {
    Button,
    ControlAdornment,
    Icon,
    Tooltip,
    type IconName,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import styles from "./IconButton.module.css";

export type IconButtonProps = Omit<
    ComponentProps<typeof Button>,
    "children" | "size"
> & {
    icon: IconName;
    label: string;
    size?: "compact" | "regular";
    edge?: "none" | "control";
};

export function IconButton({
    label,
    size = "regular",
    edge = "none",
    icon,
    className,
    disabled,
    type = "button",
    ...props
}: IconButtonProps) {
    const adornmentSize = size === "compact" ? "compact" : "regular";
    const button = (
        <Button
            {...props}
            type={type}
            size={size === "compact" ? "compactIcon" : "icon"}
            disabled={disabled}
            aria-label={label}
            className={cn(edge === "control" && styles.controlEdge, className)}
        >
            <ControlAdornment size={adornmentSize}>
                <Icon
                    name={icon}
                    size={adornmentSize === "compact" ? "sm" : "md"}
                />
            </ControlAdornment>
        </Button>
    );

    return (
        <Tooltip content={label}>
            {disabled ? (
                <span className={styles.disabledTrigger}>{button}</span>
            ) : (
                button
            )}
        </Tooltip>
    );
}
