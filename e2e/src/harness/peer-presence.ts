import type {
  DevicePresence,
  PeerInfo,
} from "../../../crates/copypaste-ui/src/generated/ipc.js";

export interface PeerPresenceSnapshot {
  readonly state: DevicePresence;
  readonly freshUntilMs: number | null;
}

export function peerPresenceSnapshot(
  peer: PeerInfo | undefined,
): PeerPresenceSnapshot | null {
  const presence = peer?.details?.presence;
  return presence === null || presence === undefined
    ? null
    : { state: presence.state, freshUntilMs: presence.fresh_until_ms };
}
