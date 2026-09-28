import type { RefCallback } from "react";

import { StateView } from "./StateView";
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
        title={title}
        className={`${styles.fallback} ${styles[size]}`}
      >
        <StateView
          mode={state === "loading" ? "loading" : "error"}
          placement="control"
          icon="imageBroken"
          aria-label={state === "loading" ? loadingLabel : failureLabel}
          role={loadingLabel || failureLabel ? "status" : "none"}
        />
      </span>
    );
  }

  return <img ref={measureRef} className={`${styles.image} ${styles[size]}`} src={image.url} alt="" title={title} draggable={false} onError={image.invalidate} />;
}
