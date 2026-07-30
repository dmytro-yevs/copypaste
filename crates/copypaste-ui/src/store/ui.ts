/**
 * Window state: which screen is showing, what is typed in the search field,
 * which row is selected, and which dismissible banners the user has dismissed.
 *
 * All of it is client-owned and none of it is persisted — routing is in-memory
 * by design (manifest §3.0), so a relaunch starts on History rather than
 * restoring a screen the user has no memory of leaving.
 *
 * Why a store rather than `useState` in `App`: the search query and the
 * selected row have to survive a trip to Settings and back, the banner queue is
 * read by the shell and written by three different screens, and prop-drilling
 * `activeId` through the list into every row is how the row memo comparator in
 * manifest §9.1 got to twenty fields. Selection is tracked **by id**, never by
 * index (INV-32) — a poll can reorder the list between a keydown and the Enter
 * that follows it.
 */
import { create } from "zustand";

export const VIEWS = ["history", "devices", "settings"] as const;
export type View = (typeof VIEWS)[number];

/** Defensive narrowing, not state recovery: anything unrecognised is History
 *  rather than a blank pane (manifest §3.0, `lib/resolveView.ts`). */
export function resolveView(value: unknown): View {
  return VIEWS.includes(value as View) ? (value as View) : "history";
}

/** Dismissible banner ids. `service-offline` is deliberately not here: it is
 *  non-dismissible (INV-17 P0). */
export type BannerId = "protocol-mismatch" | "capture-paused";

interface UiStore {
  view: View;
  query: string;
  activeId: string | null;
  dismissed: readonly BannerId[];

  setView: (view: unknown) => void;
  setQuery: (query: string) => void;
  setActiveId: (id: string | null) => void;
  dismiss: (id: BannerId) => void;
  isDismissed: (id: BannerId) => boolean;
}

export const useUi = create<UiStore>()((set, get) => ({
  view: "history",
  query: "",
  activeId: null,
  dismissed: [],

  setView: (view) => set({ view: resolveView(view) }),
  setQuery: (query) => set({ query }),
  setActiveId: (activeId) => set({ activeId }),
  // A dismissed banner stays dismissed for this window session; it is not
  // persisted, because the condition it reports is a live one and a new launch
  // should say so again.
  dismiss: (id) =>
    set((state) =>
      state.dismissed.includes(id)
        ? state
        : { dismissed: [...state.dismissed, id] },
    ),
  isDismissed: (id) => get().dismissed.includes(id),
}));
