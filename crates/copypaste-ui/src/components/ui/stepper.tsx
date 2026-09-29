import { cn } from "@/lib/cn";
import { Icon, type IconName } from "./icon";
import styles from "./stepper.module.css";

export interface StepperItem {
    id: string;
    label: string;
    stateLabel: string;
    icon: IconName;
    done?: boolean;
    current?: boolean;
}

function Stepper({
    label,
    items,
    className,
    variant = "list",
}: {
    label: string;
    items: readonly StepperItem[];
    className?: string;
    variant?: "list" | "compact";
}) {
    return (
        <ol aria-label={label} className={cn(styles.root, variant === "compact" && styles.compact, className)}>
            {items.map((item, index) => {
                return (
                    <li
                        key={item.id}
                        data-step={item.id}
                        aria-current={item.current ? "step" : undefined}
                        className={styles.item}
                    >
                        {variant === "compact" ? (
                            <span className={cn(styles.marker, item.done && styles.done)} aria-hidden="true">
                                {item.done ? <Icon name="check" /> : index + 1}
                            </span>
                        ) : (
                        <Icon
                            name={item.icon}
                            size="sm"
                            className={cn(
                                styles.icon,
                                item.done ? styles.done : styles.pending,
                            )}
                        />
                        )}
                        <span
                            className={cn(
                                styles.label,
                                item.current && styles.current,
                            )}
                        >
                            {item.label}
                        </span>
                        <span className={styles.state}>{item.stateLabel}</span>
                    </li>
                );
            })}
        </ol>
    );
}

export { Stepper };
