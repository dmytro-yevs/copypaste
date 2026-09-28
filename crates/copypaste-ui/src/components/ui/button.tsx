import {
    Children,
    Fragment,
    cloneElement,
    isValidElement,
    type ComponentProps,
    type KeyboardEvent,
    type MouseEvent,
    type ReactNode,
} from "react";
import { Slot, Slottable } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import { StateView } from "@/components/shared/StateView";
import { ControlAdornment } from "./control-adornment";
import { Icon, type IconName } from "./icon";
import { Tooltip } from "./tooltip";
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

export type ButtonProps = ComponentProps<"button"> &
    VariantProps<typeof buttonVariants> & {
        asChild?: boolean;
        icon?: IconName;
        label?: string;
        tooltip?: ReactNode;
        pending?: boolean;
        edge?: "none" | "control";
    };

function Button({
    className,
    variant,
    size,
    tone,
    state,
    asChild = false,
    children,
    disabled,
    icon,
    label,
    tooltip,
    pending = false,
    edge = "none",
    onClickCapture,
    onKeyDownCapture,
    tabIndex,
    ...props
}: ButtonProps) {
    const loading = pending || state === "loading";
    const disabledState = disabled || loading;
    const iconOnly = size === "icon" || size === "compactIcon";
    const adornmentSize = size === "compact" || size === "compactIcon" ? "compact" : "regular";
    const accessibleLabel = label ?? props["aria-label"] ?? props.title;
    const adornment = loading ? <StateView mode="loading" placement="control" /> : icon ? (
        <ControlAdornment size={adornmentSize}>
            <Icon name={icon} size={adornmentSize === "compact" ? "sm" : "md"} />
        </ControlAdornment>
    ) : null;
    const content = (
        <>
            {adornment}
            {buttonContent(children ?? (iconOnly ? undefined : label))}
        </>
    );
    const buttonClass = cn(buttonVariants({ variant, size, tone, state: loading ? "loading" : state, className }), edge === "control" && styles.controlEdge);
    const tooltipContent = tooltip ?? (iconOnly ? accessibleLabel : undefined);

    if (asChild) {
        const intrinsicButton = isValidElement(children) && children.type === "button";
        // Slot composes a child's capture handler before its own. Guard the
        // slotted child itself so disabled actions cannot run in that handler.
        const safeChild = isValidElement<{
            onClickCapture?: (event: MouseEvent<HTMLElement>) => void;
            onKeyDownCapture?: (event: KeyboardEvent<HTMLElement>) => void;
        }>(children) ? cloneElement(children, {
            onClickCapture: (event) => {
                if (disabledState) {
                    event.preventDefault();
                    event.stopPropagation();
                    return;
                }
                children.props.onClickCapture?.(event);
                onClickCapture?.(event as MouseEvent<HTMLButtonElement>);
            },
            onKeyDownCapture: (event) => {
                if (disabledState && (event.key === "Enter" || event.key === " ")) {
                    event.preventDefault();
                    event.stopPropagation();
                    return;
                }
                children.props.onKeyDownCapture?.(event);
                onKeyDownCapture?.(event as KeyboardEvent<HTMLButtonElement>);
            },
        }) : children;
        const element = (
            <Slot
                data-slot="button"
                data-state={loading ? "loading" : state ?? "normal"}
                aria-busy={loading || undefined}
                aria-disabled={disabledState || undefined}
                aria-label={accessibleLabel}
                tabIndex={disabledState ? -1 : tabIndex}
                {...(intrinsicButton && disabledState ? { disabled: true } : {})}
                data-action-size={iconOnly ? "icon" : "label"}
                className={buttonClass}
                {...props}
            >
                <Slottable child={safeChild}>
                    {(slottable) => (
                        <span
                            data-slot="button-content"
                            className={styles.content}
                        >
                            {adornment}
                            {buttonContent(slottable)}
                        </span>
                    )}
                </Slottable>
            </Slot>
        );
        return tooltipContent ? <Tooltip content={tooltipContent}>{disabledState ? <span className={styles.disabledTrigger}>{element}</span> : element}</Tooltip> : element;
    }

    const element = (
        <button
            data-slot="button"
            data-state={loading ? "loading" : state ?? "normal"}
            data-action-size={iconOnly ? "icon" : "label"}
            aria-busy={loading || undefined}
            aria-label={accessibleLabel}
            disabled={disabledState}
            tabIndex={tabIndex}
            onClickCapture={onClickCapture}
            onKeyDownCapture={onKeyDownCapture}
            type="button"
            className={buttonClass}
            {...props}
        >
            <span data-slot="button-content" className={styles.content}>
                {content}
            </span>
        </button>
    );
    return tooltipContent ? <Tooltip content={tooltipContent}>{disabledState ? <span className={styles.disabledTrigger}>{element}</span> : element}</Tooltip> : element;
}

export { Button, buttonVariants };
