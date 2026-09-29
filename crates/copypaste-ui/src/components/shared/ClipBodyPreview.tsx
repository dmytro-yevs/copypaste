import type { ReactNode } from "react";

import { fileDisplayName } from "@/lib/clipPresentation";
import { cn } from "@/lib/cn";
import type { Kind } from "@/lib/format";
import { previewLineCount, type PreviewDensitySurface } from "@/lib/previewDensity";
import { HighlightedCode } from "./HighlightedCode";
import styles from "./ClipBodyPreview.module.css";

export function ClipBodyPreview({ kind, content, previewLines, imagePreview, surface = "history" }: {
  kind: Kind;
  content: string;
  previewLines: number;
  imagePreview?: ReactNode;
  surface?: PreviewDensitySurface;
}) {
  const title = kind === "file" || kind === "path" ? fileDisplayName(content) : content;
  if (kind === "image") return <div className={styles.thumbnail}>{imagePreview}</div>;
  if (kind === "color") {
    const color = content.trim();
    return <div className={styles.color}><span aria-hidden="true" className={styles.swatch} style={{ backgroundColor: color }} /><strong>{color}</strong></div>;
  }
  if (kind === "code" || kind === "json") return <HighlightedCode content={content} kind={kind} mode="card" />;
  if (kind === "file" || kind === "path") return <div className={styles.file}><strong>{title}</strong><small>{content}</small></div>;
  if (kind === "url") return <div className={styles.link}><strong>{content.trim()}</strong></div>;
  return <p className={cn(styles.text, kind === "mail" && styles.mail)} data-preview-lines={previewLineCount(previewLines, surface)}>{title}</p>;
}
