import { useEffect, useId, useRef, useState } from "react";
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

interface PairingMutationContext {
  ceremonyId: string | null;
  epoch: number;
}

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

function pairingKey(sessionId: string, scope: string) {
  return [...PAIRING_KEY, sessionId, scope] as const;
}

export function usePairing() {
  const queryClient = useQueryClient();
  const sessionId = useId();
  const [scope, setScope] = useState("progress");
  const [lastStart, setLastStart] = useState<PairingAction | null>(null);
  const [lastAttempt, setLastAttempt] = useState<PairingAction | null>(null);
  const [presentation, setPresentation] =
    useState<PairingPresentationState | null>(null);
  const [decisionSubmitted, setDecisionSubmitted] =
    useState<"confirm" | "reject" | null>(null);
  const refreshedCeremony = useRef<string | null>(null);
  const ceremonyRef = useRef<PairingCeremony | undefined>(undefined);
  const scopeRef = useRef(scope);
  const mutationEpoch = useRef(0);
  const mounted = useRef(true);

  const command = useMutation<
    PairingCeremony,
    unknown,
    PairingAction,
    PairingMutationContext
  >({
    mutationFn: (action) => COMMANDS[action](),
    onMutate: async (action) => {
      const epoch = ++mutationEpoch.current;
      const nextScope = `mutation:${epoch}`;
      const context = {
        ceremonyId: ceremonyRef.current?.ceremony_id ?? null,
        epoch,
      };
      scopeRef.current = nextScope;
      setScope(nextScope);
      await queryClient.cancelQueries({ queryKey: [...PAIRING_KEY, sessionId] });
      setLastAttempt(action);
      if (action === "create" || action === "join") {
        setLastStart(action);
        setPresentation(null);
        setDecisionSubmitted(null);
      }
      return context;
    },
    onSuccess: (ceremony, action, context) => {
      if (!mounted.current || context.epoch !== mutationEpoch.current) return;
      const startsCeremony = action === "create" || action === "join";
      if (
        !startsCeremony &&
        context.ceremonyId !== null &&
        ceremony.ceremony_id !== null &&
        ceremony.ceremony_id !== context.ceremonyId
      ) {
        return;
      }

      const nextScope = ceremony.ceremony_id
        ? `ceremony:${ceremony.ceremony_id}:${context.epoch}`
        : `result:${context.epoch}`;
      scopeRef.current = nextScope;
      ceremonyRef.current = ceremony;
      queryClient.setQueryData(pairingKey(sessionId, nextScope), ceremony);
      setScope(nextScope);
      if (action !== "cancel" && action !== "reject") {
        setPresentation(ceremony.presentation);
      }
      if (action === "confirm" || action === "reject") {
        setDecisionSubmitted(
          ceremony.state === "awaiting_confirmation" &&
            (action === "reject" || ceremony.presentation === "presented")
            ? action
            : null,
        );
      } else if (ceremony.state !== "awaiting_confirmation") {
        setDecisionSubmitted(null);
      }
    },
  });

  const progress = useQuery<PairingCeremony>({
    queryKey: pairingKey(sessionId, scope),
    queryFn: async () => {
      const expectedScope = scope;
      const expectedCeremonyId = ceremonyRef.current?.ceremony_id ?? null;
      const ceremony = await getPairingProgress();
      if (scopeRef.current !== expectedScope) {
        return ceremonyRef.current ?? ceremony;
      }
      if (
        expectedCeremonyId !== null &&
        ceremony.ceremony_id !== null &&
        ceremony.ceremony_id !== expectedCeremonyId
      ) {
        return ceremonyRef.current ?? ceremony;
      }
      return ceremony;
    },
    enabled: !command.isPending,
    retry: false,
    staleTime: PAIRING_POLL_MS,
    refetchInterval: (query) =>
      isPairingActive(query.state.data?.state) ? PAIRING_POLL_MS : false,
  });

  const ceremony = progress.data;
  useEffect(() => {
    ceremonyRef.current = ceremony;
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

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      mutationEpoch.current += 1;
      void queryClient.cancelQueries({
        queryKey: [...PAIRING_KEY, sessionId],
      });
      if (isPairingActive(ceremonyRef.current?.state)) {
        void cancelPairing().catch(() => undefined);
      }
    };
  }, [queryClient, sessionId]);

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
