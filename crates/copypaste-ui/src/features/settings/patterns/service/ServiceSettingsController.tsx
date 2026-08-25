import {
  createContext,
  type ReactNode,
  useContext,
  useState,
} from "react";

import {
  usePrivateMode,
  useRestartService,
  useServiceConfig,
  useSetPrivateMode,
  useSetServiceConfig,
} from "@/hooks/useServiceConfig";
import type { ConfigData, ConfigPatch } from "@/lib/ipc";

type ConfigField = keyof ConfigPatch;

interface ServiceSettingsController {
  readonly data: ConfigData;
  readonly privateModeEnabled: boolean | undefined;
  readonly privateModePending: boolean;
  readonly privateModeFailed: boolean;
  readonly restartPending: boolean;
  readonly restartRequired: boolean;
  apply: (patch: ConfigPatch) => void;
  fieldFailed: (field: ConfigField) => boolean;
  fieldPending: (field: ConfigField) => boolean;
  restart: () => void;
  setPrivateMode: (enabled: boolean) => void;
}

const ServiceSettingsContext = createContext<ServiceSettingsController | null>(
  null,
);

export function useServiceSettings(): ServiceSettingsController {
  const controller = useContext(ServiceSettingsContext);
  if (controller === null) {
    throw new Error("Service settings must be rendered inside their provider");
  }
  return controller;
}

export function ServiceSettingsProvider({
  children,
  requiresPrivateMode,
}: {
  children: ReactNode;
  requiresPrivateMode: boolean;
}) {
  const config = useServiceConfig();
  const save = useSetServiceConfig();
  const privateMode = usePrivateMode(requiresPrivateMode);
  const savePrivateMode = useSetPrivateMode();
  const restartMutation = useRestartService();
  const [restartRequired, setRestartRequired] = useState(false);
  const [savingFields, setSavingFields] = useState<ReadonlySet<ConfigField>>(
    () => new Set(),
  );
  const [failedFields, setFailedFields] = useState<ReadonlySet<ConfigField>>(
    () => new Set(),
  );

  const apply = (patch: ConfigPatch) => {
    const field = Object.keys(patch)[0] as ConfigField | undefined;
    if (field !== undefined) {
      setSavingFields((current) => new Set(current).add(field));
      setFailedFields((current) => {
        const next = new Set(current);
        next.delete(field);
        return next;
      });
    }
    void save
      .mutateAsync(patch)
      .then((applied) => {
        if (applied.restart_required.length > 0) setRestartRequired(true);
      })
      .catch(() => {
        if (field !== undefined) {
          setFailedFields((current) => new Set(current).add(field));
        }
      })
      .finally(() => {
        if (field === undefined) return;
        setSavingFields((current) => {
          const next = new Set(current);
          next.delete(field);
          return next;
        });
      });
  };

  const data = config.data?.config;
  const privateModeEnabled = privateMode.data?.private_mode;
  if (data === undefined || (requiresPrivateMode && privateModeEnabled === undefined)) {
    return null;
  }

  const controller: ServiceSettingsController = {
    data,
    privateModeEnabled,
    privateModePending: savePrivateMode.isPending,
    privateModeFailed: savePrivateMode.isError,
    restartPending: restartMutation.isPending,
    restartRequired,
    apply,
    fieldFailed: (field) => failedFields.has(field),
    fieldPending: (field) => savingFields.has(field),
    restart: () =>
      restartMutation.mutate(undefined, {
        onSuccess: () => setRestartRequired(false),
      }),
    setPrivateMode: (enabled) => savePrivateMode.mutate(enabled),
  };

  return (
    <ServiceSettingsContext.Provider value={controller}>
      {children}
    </ServiceSettingsContext.Provider>
  );
}
