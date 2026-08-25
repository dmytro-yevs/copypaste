import { cn } from "@/lib/cn";
import styles from "./SkeletonText.module.css";

export type SkeletonTextWidth = "xs" | "sm" | "md" | "fill";

export function SkeletonText({
    width = "sm",
    className,
}: {
    width?: SkeletonTextWidth;
    className?: string;
}) {
    return (
        <span
            className={cn(styles.root, className)}
            data-width={width}
            aria-hidden="true"
        />
    );
}
