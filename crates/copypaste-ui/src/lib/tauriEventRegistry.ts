import {
  listen,
  type Event as TauriEvent,
  type EventCallback,
  type UnlistenFn,
} from "@tauri-apps/api/event";

import type { TauriEventName } from "@/generated/ipc";
import { hasNativeBridge } from "@/lib/ipcCall";

type AnyCallback = EventCallback<unknown>;

interface RegistryEntry {
  readonly subscribers: Set<AnyCallback>;
  pending: Promise<void> | null;
  unlisten: UnlistenFn | null;
}

const registry = new Map<TauriEventName, RegistryEntry>();

function begin(eventName: TauriEventName, entry: RegistryEntry): void {
  if (entry.pending !== null || entry.unlisten !== null) return;

  entry.pending = listen<unknown>(eventName, (event) => {
    for (const subscriber of [...entry.subscribers]) subscriber(event);
  })
    .then((unlisten) => {
      entry.pending = null;
      if (entry.subscribers.size === 0) {
        unlisten();
        if (registry.get(eventName) === entry) registry.delete(eventName);
        return;
      }
      entry.unlisten = unlisten;
    })
    .catch(() => {
      entry.pending = null;
      if (registry.get(eventName) === entry) registry.delete(eventName);
    });
}

export function subscribeNativeEvent<T>(
  eventName: TauriEventName,
  subscriber: (event: TauriEvent<T>) => void,
): () => void {
  if (!hasNativeBridge()) return () => {};

  let existing = registry.get(eventName);
  if (existing === undefined) {
    existing = { subscribers: new Set(), pending: null, unlisten: null };
    registry.set(eventName, existing);
  }
  const entry = existing;

  const callback = subscriber as unknown as AnyCallback;
  entry.subscribers.add(callback);
  begin(eventName, entry);

  let subscribed = true;
  return () => {
    if (!subscribed) return;
    subscribed = false;
    entry.subscribers.delete(callback);
    if (entry.subscribers.size > 0 || entry.pending !== null) return;
    entry.unlisten?.();
    entry.unlisten = null;
    if (registry.get(eventName) === entry) registry.delete(eventName);
  };
}
