import { useCallback, useEffect, useRef, useState } from "react";

export type PngObjectUrlState =
  | { readonly state: "absent" }
  | { readonly state: "invalid" }
  | { readonly state: "ready"; readonly url: string };

interface OwnedUrl {
  readonly url: string;
  revoked: boolean;
}

const ABSENT = { state: "absent" } as const;
const INVALID = { state: "invalid" } as const;

function revoke(owned: OwnedUrl | null): void {
  if (owned === null || owned.revoked) return;
  URL.revokeObjectURL(owned.url);
  owned.revoked = true;
}

export function usePngObjectUrl(base64?: string | null):
  PngObjectUrlState & { readonly invalidate: () => void } {
  const ownedRef = useRef<OwnedUrl | null>(null);
  const [state, setState] = useState<PngObjectUrlState>(ABSENT);

  useEffect(() => {
    if (!base64) {
      setState(ABSENT);
      return;
    }

    let owned: OwnedUrl;
    try {
      const binary = atob(base64);
      const bytes = Uint8Array.from(binary, (character) =>
        character.charCodeAt(0),
      );
      owned = {
        url: URL.createObjectURL(new Blob([bytes], { type: "image/png" })),
        revoked: false,
      };
    } catch {
      setState(INVALID);
      return;
    }

    ownedRef.current = owned;
    setState({ state: "ready", url: owned.url });
    return () => {
      revoke(owned);
      if (ownedRef.current === owned) ownedRef.current = null;
    };
  }, [base64]);

  const invalidate = useCallback(() => {
    revoke(ownedRef.current);
    ownedRef.current = null;
    setState(INVALID);
  }, []);

  return { ...state, invalidate };
}
