import { useEffect, useState } from "react";

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

function pngUrl(base64: string): string | null {
    try {
        const binary = atob(base64);
        const bytes = Uint8Array.from(binary, (character) =>
            character.charCodeAt(0),
        );
        return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
    } catch {
        return null;
    }
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
    const [url, setUrl] = useState<string | null>(null);
    const [undecodable, setUndecodable] = useState(false);

    // The Blob URL stays owned here, so it is still revoked the moment the
    // virtualizer unmounts the row; only the round trip behind it is cached.
    useEffect(() => {
        if (pngBase64 === null) {
            setUrl(null);
            setUndecodable(false);
            return;
        }
        const objectUrl = pngUrl(pngBase64);
        setUrl(objectUrl);
        setUndecodable(objectUrl === null);
        return () => {
            if (objectUrl !== null) URL.revokeObjectURL(objectUrl);
        };
    }, [pngBase64]);

    const state =
        url !== null
            ? "ready"
            : failed || undecodable
              ? "failed"
              : loading
                ? "loading"
                : "failed";

    if (state !== "ready" || url === null) {
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
            src={url}
            alt=""
            title={title}
            draggable={false}
        />
    );
}
