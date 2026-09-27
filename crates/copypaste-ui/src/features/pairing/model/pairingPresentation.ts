import type {
  PairingCeremony,
  PairingSemantics,
} from "@/lib/ipc";
import { classifyError, friendlyError, isRetryable } from "@/lib/errors";
import { t } from "@/i18n";
import { PAIRING_SEMANTICS_BY_STATE } from "@/lib/ipc";

export interface PairingPresentation {
  readonly semantics: PairingSemantics;
  readonly title: string;
  readonly detail: string;
}

export function pairingPresentation(
  ceremony: PairingCeremony | undefined,
): PairingPresentation {
  const semantics = ceremony?.semantics ?? PAIRING_SEMANTICS_BY_STATE.idle;
  return {
    semantics,
    title: semantics.copy.title,
    detail: semantics.copy.detail,
  };
}

export function pairingIsActive(ceremony: PairingCeremony | undefined): boolean {
  return ceremony?.semantics.active ?? false;
}

export interface PairingClientErrorPresentation {
  readonly title: string;
  readonly body: string;
  readonly icon: "alert";
  readonly tone: "danger";
  readonly live: "alert";
  readonly retry: boolean;
}

export function pairingClientErrorPresentation(
  error: unknown,
): PairingClientErrorPresentation | null {
  if (error === null || error === undefined) return null;
  const kind = classifyError(error);
  return {
    title: t("common.error"),
    body: friendlyError(kind),
    icon: "alert",
    tone: "danger",
    live: "alert",
    retry: kind !== "content_too_large" && kind !== "unknown" && isRetryable(error),
  };
}
