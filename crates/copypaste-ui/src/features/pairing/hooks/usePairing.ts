import { useEffect, useId, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { DISCOVERED_KEY, PEERS_KEY } from "@/hooks/useDevices";
import {
  cancelPairing,
  hasBridge,
  hasWebBridge,
  type PairingCeremony,
  type PairingPresentationState,
} from "@/lib/ipc";
import { usePairingMutations } from "@/features/pairing/hooks/usePairingMutations";
import { usePairingProgress } from "@/features/pairing/hooks/usePairingProgress";
import {
  pairingClientErrorPresentation,
  pairingPresentation,
} from "@/features/pairing/model/pairingPresentation";
import {
  inferredPairingStart,
  pairingQueryKey,
  pairingSessionKey,
  type PairingAction,
  type PairingMutationContext,
} from "@/features/pairing/model/pairingSession";

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
  const webPreview = hasWebBridge();

  const commitCeremony = (
    ceremony: PairingCeremony,
    action: PairingAction,
    context: PairingMutationContext,
  ) => {
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
    queryClient.setQueryData(pairingQueryKey(sessionId, nextScope), ceremony);
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
  };

  const beginMutation = async (action: PairingAction) => {
    const epoch = ++mutationEpoch.current;
    const nextScope = `mutation:${epoch}`;
    const context = {
      ceremonyId: ceremonyRef.current?.ceremony_id ?? null,
      epoch,
    };
    scopeRef.current = nextScope;
    setScope(nextScope);
    await queryClient.cancelQueries({ queryKey: pairingSessionKey(sessionId) });
    setLastAttempt(action);
    if (action === "create" || action === "join") {
      setLastStart(action);
      setPresentation(null);
      setDecisionSubmitted(null);
    }
    return context;
  };

  const mutations = usePairingMutations({
    webPreview,
    begin: beginMutation,
    commit: commitCeremony,
    isCurrent: (context) =>
      mounted.current && context.epoch === mutationEpoch.current,
  });
  const progress = usePairingProgress({
    enabled: !mutations.isPending,
    sessionId,
    scope,
    currentScope: () => scopeRef.current,
    currentCeremony: () => ceremonyRef.current,
  });

  const ceremony = progress.data;
  const currentPresentation = ceremony?.presentation ?? presentation;
  const protectedPresentationAvailable =
    hasBridge() && !webPreview && currentPresentation !== "unavailable";
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
        queryKey: pairingSessionKey(sessionId),
      });
      void cancelPairing().catch(() => undefined);
    };
  }, [queryClient, sessionId]);

  const error =
    mutations.previewCreateError ??
    mutations.commandError ??
    progress.error;
  const clientError = pairingClientErrorPresentation(error);
  const semantics = pairingPresentation(ceremony).semantics;
  const canRetry =
    clientError?.retry ?? (semantics.terminal && semantics.retry);

  const retry = () => {
    if (!canRetry) return;
    if (mutations.previewCreateError !== null) {
      mutations.retryPreviewCreate();
      return;
    }
    if (mutations.commandError !== null && lastAttempt === "present") {
      mutations.retryCommand("present");
      return;
    }
    if (progress.error !== null) {
      void progress.refetch();
      return;
    }
    const start = lastStart ?? inferredPairingStart(ceremony);
    if (start !== null) mutations.retryCommand(start);
  };

  return {
    webPreview,
    protectedPresentationAvailable,
    ceremony,
    error,
    isChecking: progress.isPending,
    isPending: mutations.isPending,
    pendingAction: mutations.pendingAction,
    lastAttempt,
    presentation: currentPresentation,
    decisionSubmitted,
    canRetry,
    startPreviewCreate: mutations.startPreviewCreate,
    run: mutations.run,
    retry,
  };
}

export type PairingController = ReturnType<typeof usePairing>;
