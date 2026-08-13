import type { TauriEventName } from "@/generated/ipc";

export const EVENT_CHANGED = "copypaste://changed" satisfies TauriEventName;
export const EVENT_PUSH_STATE = "copypaste://push-state" satisfies TauriEventName;
export const EVENT_CAPTURED = "copypaste://captured" satisfies TauriEventName;
export const EVENT_CAPTURE_STATE = "copypaste://capture-state" satisfies TauriEventName;
export const EVENT_PRIVATE_MODE_CHANGED =
  "private-mode-changed" satisfies TauriEventName;
export const EVENT_AUTOSTART_CHANGED =
  "autostart-changed" satisfies TauriEventName;
export const EVENT_OPEN_SETTINGS = "open-settings" satisfies TauriEventName;

export const TAURI_EVENT_NAMES = [
  EVENT_CHANGED,
  EVENT_PUSH_STATE,
  EVENT_CAPTURED,
  EVENT_CAPTURE_STATE,
  EVENT_PRIVATE_MODE_CHANGED,
  EVENT_AUTOSTART_CHANGED,
  EVENT_OPEN_SETTINGS,
] as const;

type AssertNever<T extends never> = T;
export type TauriEventNamesAreComplete = AssertNever<
  Exclude<TauriEventName, (typeof TAURI_EVENT_NAMES)[number]>
>;
