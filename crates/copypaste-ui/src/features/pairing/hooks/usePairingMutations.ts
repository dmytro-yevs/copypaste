import { useCallback, useState } from "react";
import { useMutation } from "@tanstack/react-query";

import {
  cancelPairing,
  confirmPairing,
  createPairingInvite,
  createPreviewPairingInvite,
  joinPreviewPairingInvite,
  presentPairing,
  rejectPairing,
  scanPairingInvite,
  type PairingCeremony,
  type PreviewPairingInvite,
} from "@/lib/ipc";
import type {
  PairingAction,
  PairingMutationContext,
} from "@/features/pairing/model/pairingSession";

interface PairingMutationOptions {
  webPreview: boolean;
  begin: (action: PairingAction) => Promise<PairingMutationContext>;
  commit: (
    ceremony: PairingCeremony,
    action: PairingAction,
    context: PairingMutationContext,
  ) => void;
  isCurrent: (context: PairingMutationContext) => boolean;
}

const COMMANDS: Record<PairingAction, () => Promise<PairingCeremony>> = {
  create: createPairingInvite,
  join: scanPairingInvite,
  present: presentPairing,
  confirm: confirmPairing,
  reject: rejectPairing,
  cancel: cancelPairing,
};

export function usePairingMutations({
  webPreview,
  begin,
  commit,
  isCurrent,
}: PairingMutationOptions) {
  const [previewInvite, setPreviewInvite] =
    useState<PreviewPairingInvite | null>(null);

  const command = useMutation<
    PairingCeremony,
    unknown,
    PairingAction,
    PairingMutationContext
  >({
    mutationFn: (action) => COMMANDS[action](),
    onMutate: begin,
    onSuccess: commit,
  });

  const previewCreate = useMutation<
    PreviewPairingInvite,
    unknown,
    void,
    PairingMutationContext
  >({
    mutationFn: createPreviewPairingInvite,
    onMutate: async () => {
      setPreviewInvite(null);
      return begin("create");
    },
    onSuccess: (invite, _unused, context) => {
      if (!isCurrent(context)) return;
      setPreviewInvite(invite);
      commit(invite.ceremony, "create", context);
    },
  });

  const previewJoin = useMutation<
    PairingCeremony,
    unknown,
    { code: string; addr: string },
    PairingMutationContext
  >({
    mutationFn: ({ code, addr }) => joinPreviewPairingInvite(code, addr),
    onMutate: () => begin("join"),
    onSuccess: (ceremony, _values, context) => {
      if (!isCurrent(context)) return;
      setPreviewInvite(null);
      commit(ceremony, "join", context);
    },
  });

  const isPending =
    command.isPending || previewCreate.isPending || previewJoin.isPending;
  const pendingAction = command.isPending
    ? command.variables
    : previewCreate.isPending
      ? "create"
      : previewJoin.isPending
        ? "join"
        : undefined;

  const run = (action: PairingAction) => {
    if (webPreview && action === "create") {
      previewCreate.mutate();
      return;
    }
    command.mutate(action);
  };

  return {
    commandError: command.error,
    previewCreateError: previewCreate.error,
    previewJoinError: previewJoin.error,
    previewInvite,
    isPending,
    pendingAction,
    clearPreviewInvite: useCallback(() => setPreviewInvite(null), []),
    startPreviewCreate: () => previewCreate.mutate(),
    submitPreviewJoin: (code: string, addr: string) =>
      previewJoin.mutateAsync({ code, addr }),
    retryCommand: (action: PairingAction) => command.mutate(action),
    retryPreviewCreate: () => previewCreate.mutate(),
    run,
  };
}
