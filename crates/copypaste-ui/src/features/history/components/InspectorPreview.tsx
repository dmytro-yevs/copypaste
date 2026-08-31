import type { ReactNode } from "react";

import { HighlightedCode } from "@/components/shared";
import { Icon } from "@/components/ui";
import { InspectorColorPreview } from "@/features/history/components/InspectorColorPreview";
import { clipTypeMetadata, fileDisplayName } from "@/lib/clipPresentation";
import type { Kind } from "@/lib/format";
import styles from "./InspectorPreview.module.css";

function urlName(content: string) {
    try {
        return new URL(content.trim()).hostname.replace(/^www\./, "");
    } catch {
        return content;
    }
}

export function InspectorPreview({
    kind,
    ariaLabel,
    content,
    imagePreview,
}: {
    kind: Kind;
    ariaLabel: string;
    content: string;
    imagePreview?: ReactNode;
}) {
    let preview: ReactNode;
    if (kind === "image") {
        preview = imagePreview;
    } else if (kind === "file" || kind === "path") {
        const type = clipTypeMetadata(kind, content);
        preview = (
            <div className={styles.file}>
                <Icon name={type.icon} size="sm" />
                <div>
                    <strong>{fileDisplayName(content)}</strong>
                    <small>{content}</small>
                </div>
            </div>
        );
    } else if (kind === "code" || kind === "json") {
        preview = (
            <HighlightedCode content={content} kind={kind} mode="inspector" />
        );
    } else if (kind === "color") {
        const type = clipTypeMetadata(kind);
        preview = <InspectorColorPreview label={type.label} value={content} />;
    } else if (kind === "url") {
        preview = (
            <div className={styles.link}>
                <strong>{urlName(content)}</strong>
                <span>{content}</span>
            </div>
        );
    } else {
        preview = <p className={styles.text}>{content}</p>;
    }

    return (
        <div
            className={styles.root}
            data-kind={kind}
            role="region"
            aria-label={ariaLabel}
            tabIndex={0}
        >
            {preview}
        </div>
    );
}
