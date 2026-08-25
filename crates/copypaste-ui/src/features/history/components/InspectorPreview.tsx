import type { ReactNode } from "react";

import { Icon } from "@/components/ui";
import { HighlightedCode } from "@/features/history/components/HighlightedCode";
import { InspectorColorPreview } from "@/features/history/components/InspectorColorPreview";
import { clipTypeMetadata } from "@/features/history/model/clipPresentation";
import type { Kind } from "@/lib/format";
import styles from "./InspectorPreview.module.css";

function fileName(content: string) {
    const parts = content.trim().replace(/\\/g, "/").split("/").filter(Boolean);
    return parts[parts.length - 1] || content;
}

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
                    <strong>{fileName(content)}</strong>
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
