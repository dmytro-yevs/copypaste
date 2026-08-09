import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { DISCOVERED_KEY, PEERS_KEY } from "@/hooks/useDevices";
import {
  cancelPairing,
  confirmPairing,
  createPairingInvite,
  getPairingProgress,
  type PairingCeremony,
  type PairingPresentationState,
  type PairingState,
  presentPairing,
  rejectPairing,
  scanPairingInvite,
} from "@/lib/ipc";

const PAIRING_KEY = ["pairing"] as const;
const PAIRING_POLL_MS = 700;

type PairingAction =
  | "create"
  | "join"
  | "present"
  | "confirm"
  | "reject"
  | "cancel";

const ACTIVE_STATES = new Set<PairingState>([
  "waiting_for_peer",
  "handshaking",
  "awaiting_confirmation",
]);

const COMMANDS: Record<PairingAction, () => Promise<PairingCeremony>> = {
  create: createPairingInvite,
  join: scanPairingInvite,
  present: presentPairing,
  confirm: confirmPairing,
  reject: rejectPairing,
  cancel: cancelPairing,
};

function isPairingActive(state: PairingState | undefined): boolean {
  return state !== undefined && ACTIVE_STATES.has(state);
}

function inferredStart(ceremony: PairingCeremony | undefined): PairingAction | null {
  if (ceremony?.role === "responder") return "create";
  if (ceremony?.role === "initiator") return "join";
  return null;
}

export function usePairing() {
  const queryClient = useQueryClient();
  const [lastStart, setLastStart] = useState<PairingAction | null>(null);
  const [lastAttempt, setLastAttempt] = useState<PairingAction | null>(null);
  const [presentation, setPresentation] =
    useState<PairingPresentationState | null>(null);
  const [decisionSubmitted, setDecisionSubmitted] =
    useState<"confirm" | "reject" | null>(null);
  const refreshedCeremony = useRef<string | null>(null);

  const progress = useQuery<PairingCeremony>({
    queryKey: PAIRING_KEY,
    queryFn: getPairingProgress,
    retry: false,
    refetchInterval: (query) =>
      isPairingActive(query.state.data?.state) ? PAIRING_POLL_MS : false,
  });

  const command = useMutation<PairingCeremony, unknown, PairingAction>({
    mutationFn: (action) => COMMANDS[action](),
    onMutate: (action) => {
      setLastAttempt(action);
      if (action === "create" || action === "join") {
        setLastStart(action);
        setPresentation(null);
        setDecisionSubmitted(null);
      }
    },
    onSuccess: (ceremony, action) => {
      queryClient.setQueryData(PAIRING_KEY, ceremony);
      if (action !== "cancel" && action !== "reject") {
        setPresentation(ceremony.presentation);
      }
      if (
        (action === "confirm" || action === "reject") &&
        ceremony.state === "awaiting_confirmation"
      ) {
        setDecisionSubmitted(action);
      } else if (ceremony.state !== "awaiting_confirmation") {
        setDecisionSubmitted(null);
      }
    },
  });

  const ceremony = progress.data;
  useEffect(() => {
    if (ceremony?.state !== "awaiting_confirmation") {
      setDecisionSubmitted(null);
    }
    if (ceremony?.state !== "confirmed") return;
    const refreshKey = ceremony.ceremony_id ?? "confirmed";
    if (refreshedCeremony.current === refreshKey) return;
    refreshedCeremony.current = refreshKey;
    void Promise.all([
      queryClient.invalidateQueries({ queryKey: PEERS_KEY }),
      queryClient.invalidateQueries({ queryKey: DISCOVERED_KEY }),
    ]);
  }, [ceremony, queryClient]);

  const run = (action: PairingAction) => command.mutate(action);
  const retry = () => {
    if (command.error !== null && lastAttempt !== null) {
      command.mutate(lastAttempt);
      return;
    }
    if (progress.error !== null) {
      void progress.refetch();
      return;
    }
    const start = lastStart ?? inferredStart(ceremony);
    if (start !== null) command.mutate(start);
  };

  return {
    ceremony,
    error: command.error ?? progress.error,
    isChecking: progress.isPending,
    isPending: command.isPending,
    pendingAction: command.variables,
    lastAttempt,
    presentation,
    decisionSubmitted,
    canRetry:
      command.error !== null ||
      progress.error !== null ||
      lastStart !== null ||
      inferredStart(ceremony) !== null,
    run,
    retry,
  };
}
