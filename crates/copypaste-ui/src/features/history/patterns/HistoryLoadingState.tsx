import { SkeletonText } from "@/components/shared";
import styles from "./HistoryLoadingState.module.css";

interface HistoryLoadingStateProps {
    title: string;
}

const rows = [
    "wide",
    "medium",
    "short",
    "wide",
    "medium",
    "wide",
    "short",
    "medium",
    "wide",
    "short",
    "wide",
    "medium",
] as const;

export function HistoryLoadingState({ title }: HistoryLoadingStateProps) {
    return (
        <section
            className={styles.root}
            role="status"
            aria-label={title}
            aria-live="polite"
            aria-busy="true"
        >
            <div className={styles.list} aria-hidden="true">
                {rows.map((width, index) => (
                    <div className={styles.slot} key={`${width}-${index}`}>
                        <div className={styles.card} data-width={width}>
                            <div className={styles.content}>
                                <div className={styles.body}>
                                    <SkeletonText
                                        width={width === "wide" ? "fill" : width === "medium" ? "md" : "sm"}
                                    />
                                    <SkeletonText width={width === "short" ? "xs" : "sm"} />
                                </div>
                                <div className={styles.metaRoot}>
                                    <div className={styles.meta}>
                                        <span className={styles.typeGlyph} />
                                        <span className={styles.appGlyph} />
                                        <SkeletonText width="xs" />
                                        <span className={styles.metaDot} />
                                        <SkeletonText width="xs" />
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                ))}
            </div>
        </section>
    );
}
