import { useSyncExternalStore } from "react";

import { usePrefs } from "@/store/prefs";

export function usePrefsHydrated(): boolean {
  const hydrated = useSyncExternalStore(
    (onChange) => usePrefs.persist.onFinishHydration(onChange),
    () => usePrefs.persist.hasHydrated(),
    () => true,
  );
  return import.meta.env.MODE === "test" || hydrated;
}
