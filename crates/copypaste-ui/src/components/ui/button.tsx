import {
    Children,
    Fragment,
    isValidElement,
    type ComponentProps,
    type ReactNode,
} from "react";
import { Slot, Slottable } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./button.module.css";

const buttonVariants = cva(styles.button, {
    variants: {
        variant: {
            primary: styles.primary,
            secondary: styles.secondary,
            ghost: styles.ghost,
            danger: styles.danger,
        },
        size: {
            compact: styles.compact,
            compactIcon: styles.compactIcon,
            sm: styles.sm,
            md: styles.md,
            lg: styles.lg,
            icon: styles.icon,
        },
        tone: {
            neutral: undefined,
            danger: styles.dangerTone,
        },
        state: { normal: undefined, loading: styles.loading },
    },
    defaultVariants: {
        variant: "primary",
        size: "md",
        tone: "neutral",
        state: "normal",
    },
});

function buttonContent(children: ReactNode): ReactNode {
    return Children.map(children, (child) => {
        if (typeof child === "string" || typeof child === "number") {
            return <span data-slot="button-label">{child}</span>;
        }
        if (
            isValidElement<{ children?: ReactNode }>(child) &&
            child.type === Fragment
        ) {
            return buttonContent(child.props.children);
        }
        return child;
    });
}

function Button({
    className,
    variant,
    size,
    tone,
    state,
    asChild = false,
    children,
    disabled,
    ...props
}: ComponentProps<"button"> &
    VariantProps<typeof buttonVariants> & { asChild?: boolean }) {
    const loading = state === "loading";
    const disabledState = disabled || loading;

    if (asChild) {
        return (
            <Slot
                data-slot="button"
                data-state={state ?? "normal"}
                aria-busy={loading || undefined}
                aria-disabled={disabledState || undefined}
                className={cn(
                    buttonVariants({ variant, size, tone, state, className }),
                )}
                {...props}
            >
                <Slottable child={children}>
                    {(slottable) => (
                        <span
                            data-slot="button-content"
                            className={styles.content}
                        >
                            {buttonContent(slottable)}
                        </span>
                    )}
                </Slottable>
            </Slot>
        );
    }

    return (
        <button
            data-slot="button"
            data-state={state ?? "normal"}
            aria-busy={loading || undefined}
            disabled={disabledState}
            className={cn(
                buttonVariants({ variant, size, tone, state, className }),
            )}
            {...props}
        >
            <span data-slot="button-content" className={styles.content}>
                {buttonContent(children)}
            </span>
        </button>
    );
}

export { Button, buttonVariants };
