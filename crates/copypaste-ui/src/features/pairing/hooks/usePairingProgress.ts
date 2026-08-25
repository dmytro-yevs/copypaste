import { useQuery } from "@tanstack/react-query";

import {
  isPairingActive,
  PAIRING_POLL_MS,
  pairingQueryKey,
} from "@/features/pairing/model/pairingSession";
import { getPairingProgress, type PairingCeremony } from "@/lib/ipc";

interface PairingProgressOptions {
  enabled: boolean;
  sessionId: string;
  scope: string;
  currentScope: () => string;
  currentCeremony: () => PairingCeremony | undefined;
}

export function usePairingProgress({
  enabled,
  sessionId,
  scope,
  currentScope,
  currentCeremony,
}: PairingProgressOptions) {
  return useQuery<PairingCeremony>({
    queryKey: pairingQueryKey(sessionId, scope),
    queryFn: async () => {
      const expectedCeremonyId = currentCeremony()?.ceremony_id ?? null;
      const ceremony = await getPairingProgress();
      if (currentScope() !== scope) return currentCeremony() ?? ceremony;
      if (
        expectedCeremonyId !== null &&
        ceremony.ceremony_id !== null &&
        ceremony.ceremony_id !== expectedCeremonyId
      ) {
        return currentCeremony() ?? ceremony;
      }
      return ceremony;
    },
    enabled,
    retry: false,
    staleTime: PAIRING_POLL_MS,
    refetchInterval: (query) =>
      isPairingActive(query.state.data?.state) ? PAIRING_POLL_MS : false,
  });
}
