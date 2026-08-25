import { useCallback, useState } from "react";

import {
  useCloudSetEndpoint,
  useCloudSignIn,
  useCloudSignOut,
  useCloudSignUp,
  useCloudStatus,
  useCloudSyncNow,
} from "@/hooks/useCloud";

interface EndpointDraft {
  readonly url: string;
  readonly anonKey: string;
}

interface AccountDraft {
  readonly email: string;
  readonly password: string;
  readonly passphrase: string;
}

const EMPTY_ENDPOINT: EndpointDraft = { url: "", anonKey: "" };
const EMPTY_ACCOUNT: AccountDraft = { email: "", password: "", passphrase: "" };

export type EndpointField = keyof EndpointDraft;
export type AccountField = keyof AccountDraft;

export function useCloudAccountController() {
  const cloud = useCloudStatus();
  const setEndpoint = useCloudSetEndpoint();
  const signIn = useCloudSignIn();
  const signUp = useCloudSignUp();
  const signOut = useCloudSignOut();
  const syncNow = useCloudSyncNow();
  const [endpointDraft, setEndpointDraft] = useState(EMPTY_ENDPOINT);
  const [accountDraft, setAccountDraft] = useState(EMPTY_ACCOUNT);
  const [endpointEditorOpen, setEndpointEditorOpen] = useState(false);

  const updateEndpoint = useCallback((field: EndpointField, value: string) => {
    setEndpoint.reset();
    setEndpointDraft((draft) => ({ ...draft, [field]: value }));
  }, [setEndpoint]);

  const updateAccount = useCallback((field: AccountField, value: string) => {
    signIn.reset();
    signUp.reset();
    setAccountDraft((draft) => ({ ...draft, [field]: value }));
  }, [signIn, signUp]);

  const clearEndpoint = useCallback(() => {
    setEndpoint.reset();
    setEndpointDraft(EMPTY_ENDPOINT);
  }, [setEndpoint]);

  const clearAccount = useCallback(() => {
    signIn.reset();
    signUp.reset();
    setAccountDraft(EMPTY_ACCOUNT);
  }, [signIn, signUp]);

  const finishEndpointChange = useCallback(() => {
    signIn.reset();
    signUp.reset();
    setEndpointDraft(EMPTY_ENDPOINT);
    setAccountDraft(EMPTY_ACCOUNT);
    setEndpointEditorOpen(false);
  }, [signIn, signUp]);

  const configureEndpoint = useCallback(() => {
    const url = endpointDraft.url.trim();
    const anonKey = endpointDraft.anonKey.trim();
    if (!url || !anonKey) return;
    setEndpoint.mutate({ url, anonKey }, { onSuccess: finishEndpointChange });
  }, [endpointDraft, finishEndpointChange, setEndpoint]);

  const restoreHostedEndpoint = useCallback(() => {
    setEndpoint.mutate(
      { url: "", anonKey: "" },
      { onSuccess: finishEndpointChange },
    );
  }, [finishEndpointChange, setEndpoint]);

  const submitSignIn = useCallback(() => {
    const email = accountDraft.email.trim();
    if (!email || !accountDraft.password || !accountDraft.passphrase) return;
    signUp.reset();
    signIn.mutate(
      { ...accountDraft, email },
      { onSuccess: clearAccount },
    );
  }, [accountDraft, clearAccount, signIn, signUp]);

  const submitSignUp = useCallback(() => {
    const email = accountDraft.email.trim();
    if (!email || !accountDraft.password || !accountDraft.passphrase) return;
    signIn.reset();
    signUp.mutate(
      { ...accountDraft, email },
      { onSuccess: clearAccount },
    );
  }, [accountDraft, clearAccount, signIn, signUp]);

  const submitSignOut = useCallback(() => {
    signOut.mutate(undefined, { onSuccess: clearAccount });
  }, [clearAccount, signOut]);

  const cancelEndpointEdit = useCallback(() => {
    clearEndpoint();
    setEndpointEditorOpen(false);
  }, [clearEndpoint]);

  const busy = setEndpoint.isPending || signIn.isPending || signUp.isPending
    || signOut.isPending || syncNow.isPending;

  return {
    cloud,
    status: cloud.data,
    endpointDraft,
    endpointEditorOpen,
    endpointDirty: Boolean(endpointDraft.url || endpointDraft.anonKey),
    endpointComplete: Boolean(endpointDraft.url.trim() && endpointDraft.anonKey.trim()),
    endpointPending: setEndpoint.isPending,
    endpointError: setEndpoint.isError,
    updateEndpoint,
    clearEndpoint,
    openEndpointEditor: () => setEndpointEditorOpen(true),
    cancelEndpointEdit,
    configureEndpoint,
    restoreHostedEndpoint,
    accountDraft,
    accountDirty: Boolean(accountDraft.email || accountDraft.password || accountDraft.passphrase),
    accountComplete: Boolean(
      accountDraft.email.trim() && accountDraft.password && accountDraft.passphrase,
    ),
    accountAction: signIn.isPending ? "sign-in" as const
      : signUp.isPending ? "sign-up" as const
        : null,
    accountError: signIn.isError ? "sign-in" as const
      : signUp.isError ? "sign-up" as const
        : null,
    updateAccount,
    clearAccount,
    submitSignIn,
    submitSignUp,
    signOutPending: signOut.isPending,
    signOutError: signOut.isError,
    submitSignOut,
    syncPending: syncNow.isPending,
    syncError: syncNow.isError,
    syncNow: () => syncNow.mutate(),
    busy,
  };
}

export type CloudAccountController = ReturnType<typeof useCloudAccountController>;
