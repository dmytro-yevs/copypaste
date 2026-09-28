import type { ReactNode } from "react";

import { StateView } from "@/components/shared/StateView";
import type { ConfigPatch } from "@/lib/ipc";
import { useServiceSettings } from "./ServiceSettingsController";

export function ServiceFieldNote({
  children,
  field,
}: {
  children?: ReactNode;
  field: keyof ConfigPatch;
}) {
  const controller = useServiceSettings();
  return (
    <>
      {controller.fieldPending(field) ? (
        <StateView mode="loading" placement="control" title="Saving…" />
      ) : controller.fieldFailed(field) ? (
        <StateView mode="error" placement="control" title="This change wasn’t saved." />
      ) : null}
      {children}
    </>
  );
}
