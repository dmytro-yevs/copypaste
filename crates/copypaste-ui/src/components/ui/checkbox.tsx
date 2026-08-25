import type { ComponentProps } from "react";
import * as CheckboxPrimitive from "@radix-ui/react-checkbox";

import { cn } from "@/lib/cn";
import { Icon } from "./icon";
import styles from "./checkbox.module.css";

type CheckboxProps = Omit<
    ComponentProps<typeof CheckboxPrimitive.Root>,
    "slot"
> & {
    hitSize?: "compact" | "comfortable";
};

function CheckboxRoot({
    slot,
    className,
    hitSize = "compact",
    ...props
}: CheckboxProps & { slot: "checkbox" | "selection-control" }) {
    return (
        <CheckboxPrimitive.Root
            data-slot={slot}
            className={cn(styles.root, styles[hitSize], className)}
            {...props}
        >
            <span data-slot="checkbox-control" className={styles.control}>
                <CheckboxPrimitive.Indicator
                    data-slot="checkbox-indicator"
                    className={styles.indicator}
                >
                    <Icon
                        name="check"
                        size="xs"
                        className={styles.check}
                        weight="bold"
                    />
                </CheckboxPrimitive.Indicator>
            </span>
        </CheckboxPrimitive.Root>
    );
}

function Checkbox(props: CheckboxProps) {
    return <CheckboxRoot slot="checkbox" {...props} />;
}

function SelectionControl(props: CheckboxProps) {
    return <CheckboxRoot slot="selection-control" {...props} />;
}

export { Checkbox, SelectionControl };
