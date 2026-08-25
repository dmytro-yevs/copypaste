import type { ReactNode } from "react";

import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import styles from "./IllustratedErrorState.module.css";

interface IllustratedErrorStateProps {
    title: string;
    body: string;
    actions: ReactNode;
    compact?: boolean;
    className?: string;
}

export function RepairBotArtwork() {
    return (
        <svg
            className={styles.artwork}
            viewBox="0 0 288 188"
            aria-hidden="true"
            focusable="false"
        >
            <ellipse className={styles.halo} cx="144" cy="100" rx="104" ry="69" />
            <path className={styles.ground} d="M37 151h214" />
            <g className={styles.cable}>
                <path d="M20 105c18 0 21 20 43 20h18" />
                <path d="M207 125h18c22 0 25-20 43-20" />
            </g>
            <g className={styles.plugLeft}>
                <path d="M65 114h19v22H65a7 7 0 0 1-7-7v-8a7 7 0 0 1 7-7Z" />
                <path d="M84 120h8m-8 10h8" />
            </g>
            <g className={styles.plugRight}>
                <path d="M223 114h-19v22h19a7 7 0 0 0 7-7v-8a7 7 0 0 0-7-7Z" />
                <path d="M204 120h-8m8 10h-8" />
            </g>
            <g className={styles.bot}>
                <path className={styles.antenna} d="M144 42V28m0 0 8-8m-8 8-8-8" />
                <rect className={styles.chassis} x="96" y="43" width="96" height="101" rx="27" />
                <rect className={styles.face} x="112" y="58" width="64" height="33" rx="13" />
                <circle className={styles.eye} cx="130" cy="74" r="3.5" />
                <circle className={styles.eye} cx="158" cy="74" r="3.5" />
                <path className={styles.mouth} d="M138 82h12" />
                <rect className={styles.clipboard} x="118" y="97" width="52" height="37" rx="8" />
                <path className={styles.clip} d="M136 97v-4h16v4" />
                <path className={styles.clipLines} d="M128 108h32m-32 8h24m-24 8h28" />
                <path className={styles.armLeft} d="m99 81-19 13 4 20" />
                <g className={styles.tool}>
                    <path d="m84 110-12 15" />
                    <path d="m67 122-5 9 7 6 8-6" />
                </g>
                <path className={styles.armRight} d="m189 82 18 15-7 18" />
                <path d="M124 144v9m40-9v9" />
            </g>
            <g className={styles.signal}>
                <path d="m91 105 10-6m86 0 10 6" />
                <path d="m92 115 9-2m86 0 9 2" />
            </g>
            <g className={styles.errorSignal}>
                <circle cx="210" cy="63" r="8" />
                <path d="M210 59v5m0 4h.01" />
            </g>
        </svg>
    );
}

export function IllustratedErrorState({
    title,
    body,
    actions,
    compact = false,
    className,
}: IllustratedErrorStateProps) {
    const { t } = useTranslation();

    return (
        <section
            className={cn(styles.root, compact && styles.compact, className)}
            role="alert"
        >
            <div className={styles.layout}>
                <div className={styles.artworkWell}>
                    <RepairBotArtwork />
                </div>
                <div className={styles.content}>
                    <p className={styles.kicker}>{t("common.error")}</p>
                    <h2 className={styles.title}>{title}</h2>
                    <p className={styles.body}>{body}</p>
                    <div className={styles.actions}>
                        <div className={styles.actionLayout}>{actions}</div>
                    </div>
                </div>
            </div>
        </section>
    );
}
