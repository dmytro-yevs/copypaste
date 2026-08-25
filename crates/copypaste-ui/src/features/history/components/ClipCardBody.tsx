import type { ReactNode } from "react";

import { ClipThumbnail } from "@/features/history/components/ClipThumbnail";
import { ColorClipPreview } from "@/features/history/components/ColorClipPreview";
import { HighlightedCode } from "@/features/history/components/HighlightedCode";
import { fileDisplayName } from "@/features/history/model/clipPresentation";
import { cn } from "@/lib/cn";
import type { Kind } from "@/lib/format";
import { previewLineCount } from "@/lib/previewDensity";
import styles from "./ClipCardBody.module.css";

export function ClipCardBody({
  kind,
  content,
  masked = false,
  previewLines,
  imagePreview,
}: {
  kind: Kind;
  content: string;
  masked?: boolean;
  previewLines: number;
  imagePreview?: ReactNode;
}) {
  const title =
    kind === "file" || kind === "path"
      ? fileDisplayName(content)
      : content;

  if (masked) {
    return <div className={styles.secret} aria-hidden="true"><i /><i /><i /></div>;
  }
  if (kind === "image") {
    return <ClipThumbnail kind="image" image={imagePreview} />;
  }
  if (kind === "color") {
    return <ColorClipPreview value={content} />;
  }
  if (kind === "code" || kind === "json") {
    return <HighlightedCode content={content} kind={kind} mode="card" />;
  }
  if (kind === "file" || kind === "path") {
    return (
      <div className={styles.file}>
        <strong>{title}</strong>
        <small>{content}</small>
      </div>
    );
  }
  if (kind === "url") {
    return (
      <div className={styles.link}>
        <strong>{content.trim()}</strong>
      </div>
    );
  }
  return (
    <p
      className={cn(styles.text, kind === "mail" && styles.mail)}
      data-preview-lines={previewLineCount(previewLines)}
    >
      {title}
    </p>
  );
}
