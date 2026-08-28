import type { IconName } from "@/components/ui";
import type {
  PairingCeremony,
  PairingMessageId,
  PairingSemantics,
} from "@/lib/ipc";

export interface PairingPresentation {
  readonly semantics: PairingSemantics;
  readonly titleKey: `devices.pairing.semantic.${PairingMessageId}.title`;
  readonly bodyKey:
    | `devices.pairing.semantic.${PairingMessageId}.body`
    | "devices.pairing.semantic.paired.device";
  readonly deviceName?: string;
  readonly icon: IconName;
}

const IDLE_SEMANTICS: PairingSemantics = {
  message_id: "ready",
  icon: "shieldCheck",
  tone: "neutral",
  live: "status",
  active: false,
  terminal: false,
  needs_devices: false,
  review_secure: false,
  retry: false,
};

export function pairingPresentation(
  ceremony: PairingCeremony | undefined,
): PairingPresentation {
  const semantics = ceremony?.semantics ?? IDLE_SEMANTICS;
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
