import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";
import styles from "./ActiveControlBadge.module.css";

interface ActiveControlBadgeProps extends ComponentProps<"span"> {
    active: boolean;
}

export function ActiveControlBadge({
    active,
    children,
    className,
    ...props
}: ActiveControlBadgeProps) {
    return (
        <span
            {...props}
            data-slot="active-control"
            data-active-control={active || undefined}
            className={cn(styles.root, className)}
        >
            {children}
            {active ? (
                <span
                    aria-hidden="true"
                    data-slot="active-control-indicator"
                    className={styles.indicator}
                />
            ) : null}
        </span>
    );
}
