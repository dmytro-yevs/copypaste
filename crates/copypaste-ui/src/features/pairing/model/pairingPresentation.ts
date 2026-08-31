import type { IconName } from "@/components/ui";
import type {
  PairingCeremony,
  PairingMessageId,
  PairingSemantics,
} from "@/lib/ipc";
import { classifyError, friendlyError, isRetryable } from "@/lib/errors";
import { t } from "@/i18n";
import { PAIRING_SEMANTICS_BY_STATE } from "@/lib/ipc";

export interface PairingPresentation {
  readonly semantics: PairingSemantics;
  readonly titleKey: `devices.pairing.semantic.${PairingMessageId}.title`;
  readonly bodyKey:
    | `devices.pairing.semantic.${PairingMessageId}.body`
    | "devices.pairing.semantic.paired.device";
  readonly deviceName?: string;
  readonly icon: IconName;
}

export function pairingPresentation(
  ceremony: PairingCeremony | undefined,
): PairingPresentation {
  const semantics = ceremony?.semantics ?? PAIRING_SEMANTICS_BY_STATE.idle;
  const prefix = `devices.pairing.semantic.${semantics.message_id}` as const;
  const deviceName = ceremony?.known_device?.name;
  return {
    semantics,
    titleKey: `${prefix}.title`,
    bodyKey:
      semantics.message_id === "paired" && deviceName !== undefined
        ? "devices.pairing.semantic.paired.device"
        : `${prefix}.body`,
    deviceName,
    icon: semantics.icon,
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
