import { create } from "zustand";

export type ViewId = "history" | "devices" | "settings" | "about";

interface UIState {
  view: ViewId;
  setView: (view: ViewId) => void;
}

export const useUI = create<UIState>((set) => ({
  view: "history",
  setView: (view) => set({ view })
}));
