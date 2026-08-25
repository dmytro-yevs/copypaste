import { usePngObjectUrl } from "@/components/shared";
import { Icon } from "@/components/ui";

import styles from "./ClipImage.module.css";

export interface ClipImageProps {
    pngBase64: string | null;
    loading: boolean;
    failed: boolean;
    title?: string;
    loadingLabel?: string;
    failureLabel?: string;
    size?: "intrinsic" | "thumbnail" | "fill" | "detail" | "quickPaste";
}

/** The virtual list unmounts this outside its visible window, which also drops
 * its Blob URL instead of retaining image data for the whole history. */
export function ClipImage({
    pngBase64,
    loading,
    failed,
    title,
    loadingLabel,
    failureLabel,
    size = "intrinsic",
}: ClipImageProps) {
    const image = usePngObjectUrl(pngBase64);

    const state =
        image.state === "ready"
            ? "ready"
            : failed || image.state === "invalid"
              ? "failed"
              : loading
                ? "loading"
                : "failed";

    if (state !== "ready" || image.state !== "ready") {
        return (
            <span
                aria-label={state === "loading" ? loadingLabel : failureLabel}
                role={loadingLabel || failureLabel ? "status" : undefined}
                title={title}
                className={`${styles.fallback} ${styles[size]}`}
            >
                {state === "loading" ? (
                    <Icon name="spinner" size="sm" className={styles.spinner} />
                ) : (
                    <Icon name="imageBroken" size="sm" />
                )}
            </span>
        );
    }

    return (
        <img
            className={`${styles.image} ${styles[size]}`}
            src={image.url}
            alt=""
            title={title}
            draggable={false}
            onError={image.invalidate}
        />
    );
}
