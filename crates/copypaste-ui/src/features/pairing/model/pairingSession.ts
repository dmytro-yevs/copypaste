import type { PairingCeremony } from "@/lib/ipc";

export const PAIRING_POLL_MS = 700;

export type PairingAction =
  | "create"
  | "join"
  | "present"
  | "confirm"
  | "reject"
  | "cancel";

export interface PairingMutationContext {
  ceremonyId: string | null;
  epoch: number;
}

export function inferredPairingStart(
  ceremony: PairingCeremony | undefined,
): PairingAction | null {
  if (ceremony?.role === "responder") return "create";
  if (ceremony?.role === "initiator") return "join";
  return null;
}

export function pairingSessionKey(sessionId: string) {
  return ["pairing", sessionId] as const;
}

export function pairingQueryKey(sessionId: string, scope: string) {
  return [...pairingSessionKey(sessionId), scope] as const;
}
