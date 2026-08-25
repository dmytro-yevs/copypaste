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
}: {
    label: string;
    items: readonly StepperItem[];
    className?: string;
}) {
    return (
        <ol aria-label={label} className={cn(styles.root, className)}>
            {items.map((item) => {
                return (
                    <li
                        key={item.id}
                        data-step={item.id}
                        className={styles.item}
                    >
                        <Icon
                            name={item.icon}
                            size="sm"
                            className={cn(
                                styles.icon,
                                item.done ? styles.done : styles.pending,
                            )}
                        />
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
