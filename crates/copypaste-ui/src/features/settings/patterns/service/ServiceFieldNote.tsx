import type { ReactNode } from "react";

import { FieldFeedback } from "@/components/shared";
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
        <FieldFeedback state="pending">Saving…</FieldFeedback>
      ) : controller.fieldFailed(field) ? (
        <FieldFeedback state="error">
          This change wasn’t saved.
        </FieldFeedback>
      ) : null}
      {children}
    </>
  );
}
