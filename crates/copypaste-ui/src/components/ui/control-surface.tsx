import type { ComponentProps, ReactNode } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./control-surface.module.css";

export const controlSurfaceVariants = cva(styles.root, {
    variants: {
        size: {
            compact: styles.compact,
            sm: styles.sm,
            md: styles.md,
            library: styles.library,
        },
        width: { content: styles.content, fill: styles.fill },
        state: {
            normal: undefined,
            invalid: styles.invalid,
            disabled: styles.disabled,
        },
    },
    defaultVariants: { size: "md", width: "content", state: "normal" },
});

export type ControlSurfaceVariants = VariantProps<
    typeof controlSurfaceVariants
>;

export function ControlSurface({
    className,
    size,
    width,
    state,
    ...props
}: ComponentProps<"div"> & ControlSurfaceVariants) {
    return (
        <div
            data-slot="control-surface"
            data-state={state ?? "normal"}
            className={cn(
                controlSurfaceVariants({ size, width, state }),
                className,
            )}
            {...props}
        />
    );
}

export function ControlEndSlot({ children }: { children?: ReactNode }) {
    return (
        <span data-slot="control-end-slot" className={styles.endSlot}>
            {children}
        </span>
    );
}
