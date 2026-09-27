import { Icon } from "@/components/ui";
import type { RefCallback } from "react";

import { usePngObjectUrl } from "./usePngObjectUrl";
import styles from "./ClipImage.module.css";

export interface ClipImageProps {
  pngBase64: string | null;
  loading: boolean;
  failed: boolean;
  title?: string;
  loadingLabel?: string;
  failureLabel?: string;
  size?: "intrinsic" | "thumbnail" | "fill" | "detail" | "quickPaste";
  measureRef?: RefCallback<HTMLElement>;
}

export function ClipImage({
  pngBase64,
  loading,
  failed,
  title,
  loadingLabel,
  failureLabel,
  size = "intrinsic",
  measureRef,
}: ClipImageProps) {
  const image = usePngObjectUrl(pngBase64);
  const state = image.state === "ready"
    ? "ready"
    : failed || image.state === "invalid"
      ? "failed"
      : loading
        ? "loading"
        : "failed";

  if (state !== "ready" || image.state !== "ready") {
    return (
      <span
        ref={measureRef}
        aria-label={state === "loading" ? loadingLabel : failureLabel}
        role={loadingLabel || failureLabel ? "status" : undefined}
        title={title}
        className={`${styles.fallback} ${styles[size]}`}
      >
        {state === "loading" ? <Icon name="spinner" size="sm" className={styles.spinner} /> : <Icon name="imageBroken" size="sm" />}
      </span>
    );
  }

  return <img ref={measureRef} className={`${styles.image} ${styles[size]}`} src={image.url} alt="" title={title} draggable={false} onError={image.invalidate} />;
}
