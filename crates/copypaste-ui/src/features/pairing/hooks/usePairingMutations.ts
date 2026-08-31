import { useMutation } from "@tanstack/react-query";

import {
  cancelPairing,
  confirmPairing,
  createPairingInvite,
  createPreviewPairingInvite,
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
    onMutate: async () => begin("create"),
    onSuccess: (invite, _unused, context) => {
      if (!isCurrent(context)) return;
      commit(invite.ceremony, "create", context);
    },
  });

  const isPending = command.isPending || previewCreate.isPending;
  const pendingAction = command.isPending
    ? command.variables
    : previewCreate.isPending
      ? "create"
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
    isPending,
    pendingAction,
    startPreviewCreate: () => previewCreate.mutate(),
    retryCommand: (action: PairingAction) => command.mutate(action),
    retryPreviewCreate: () => previewCreate.mutate(),
    run,
  };
}
