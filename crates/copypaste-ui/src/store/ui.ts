/**
 * Window state, none of it persisted: routing is in-memory by design (§3.0), so
 * a relaunch starts on History rather than restoring a screen the user has no
 * memory of leaving.
 *
 * Selection is tracked **by id**, never by index (INV-32): a poll can reorder
 * the list between a keydown and the Enter that follows it (CopyPaste-8ebg.17).
 */
import { create } from "zustand";

/** `capture` is reachable but not navigable: it is opened from the status strip
 *  and from the loss notification's re-arm request, and putting it in the tab
 *  bar would give macOS — which has no ladder — a screen with nothing on it. */
export const VIEWS = ["history", "devices", "settings", "capture"] as const;
export type View = (typeof VIEWS)[number];

/** Defensive narrowing, not state recovery: anything unrecognised is History
 *  rather than a blank pane. */
export function resolveView(value: unknown): View {
  return VIEWS.includes(value as View) ? (value as View) : "history";
}

/** `service-offline` and `history-unreadable` are deliberately absent: both are
 *  non-dismissible (INV-17), because nothing in the app works while they hold. */
export type BannerId = "protocol-mismatch" | "capture-paused";

interface UiStore {
  view: View;
  settingsTab: string | null;
  query: string;
  activeId: string | null;
  dismissed: readonly BannerId[];

  setView: (view: unknown) => void;
  setSettingsTab: (tab: string | null) => void;
  setQuery: (query: string) => void;
  setActiveId: (id: string | null) => void;
  dismiss: (id: BannerId) => void;
  isDismissed: (id: BannerId) => boolean;
}

export const useUi = create<UiStore>()((set, get) => ({
  view: "history",
  settingsTab: null,
  query: "",
  activeId: null,
  dismissed: [],

  setView: (view) => set({ view: resolveView(view) }),
  setSettingsTab: (settingsTab) => set({ settingsTab }),
  setQuery: (query) => set({ query }),
  setActiveId: (activeId) => set({ activeId }),
  // Dismissed for this session only: the condition is live, and a new launch
  // should say so again.
  dismiss: (id) =>
    set((state) =>
      state.dismissed.includes(id)
        ? state
        : { dismissed: [...state.dismissed, id] },
    ),
  isDismissed: (id) => get().dismissed.includes(id),
}));
