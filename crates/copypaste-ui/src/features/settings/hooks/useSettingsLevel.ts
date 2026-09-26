import { useCallback, useEffect, useRef } from "react";

const HISTORY_KEY = "copypasteSettingsLevel";
let nextOwner = 0;
let pendingRetirement: Promise<void> | null = null;

interface SettingsHistoryMarker {
  owner: string;
  path: string[];
}

function marker(state: unknown): SettingsHistoryMarker | null {
  if (state === null || typeof state !== "object") return null;
  const value = (state as Record<string, unknown>)[HISTORY_KEY];
  if (value === null || typeof value !== "object") return null;
  const candidate = value as Record<string, unknown>;
  if (typeof candidate.owner !== "string" ||
      !Array.isArray(candidate.path) ||
      !candidate.path.every((part) => typeof part === "string")) return null;
  return candidate as unknown as SettingsHistoryMarker;
}

function withoutMarker(state: unknown): unknown {
  if (state === null || typeof state !== "object" || Array.isArray(state)) return state;
  const copy = { ...state } as Record<string, unknown>;
  delete copy[HISTORY_KEY];
  return copy;
}

function withMarker(state: unknown, owner: string, path: readonly string[]) {
  const base = state !== null && typeof state === "object" && !Array.isArray(state)
    ? state
    : {};
  return { ...base, [HISTORY_KEY]: { owner, path: [...path] } };
}

function samePath(left: readonly string[], right: readonly string[]) {
  return left.length === right.length &&
    left.every((part, index) => part === right[index]);
}

function retire(owner: string, baseState: unknown, navigationPending: boolean) {
  const current = marker(window.history.state);
  if (current?.owner !== owner) return;
  if (current.path.length === 0) {
    window.history.replaceState(baseState, "");
    return;
  }

  const retirement = new Promise<void>((resolve) => {
    const finish = () => {
      window.removeEventListener("popstate", onPop);
      resolve();
    };
    const onPop = (event: PopStateEvent) => {
      const reached = marker(event.state);
      if (reached?.owner !== owner) {
        finish();
      } else if (reached.path.length === 0) {
        window.history.replaceState(baseState, "");
        finish();
      } else {
        window.history.go(-reached.path.length);
      }
    };
    window.addEventListener("popstate", onPop);
    if (!navigationPending) window.history.go(-current.path.length);
  });
  pendingRetirement = retirement;
  void retirement.finally(() => {
    if (pendingRetirement === retirement) pendingRetirement = null;
  });
}

/** WryActivity sends Android Back through WebView history. Each compact page
 * needs an owned entry, and an unspent entry must be retired on rotation or
 * leaving Settings so a later Back does not appear to do nothing. */
export function useSettingsLevel(
  path: readonly string[],
  onNavigate: (path: readonly string[]) => void,
): () => void {
  const owner = useRef<string | null>(null);
  if (owner.current === null) owner.current = `settings-${++nextOwner}`;
  const desired = useRef(path);
  desired.current = path;
  const navigate = useRef(onNavigate);
  navigate.current = onNavigate;
  const session = useRef<{ sync: () => void; back: () => void; dispose: () => void } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const sessionOwner = owner.current!;
    const initialize = () => {
      if (cancelled) return;
      const baseState = withoutMarker(window.history.state);
      window.history.replaceState(withMarker(baseState, sessionOwner, []), "");
      let currentPath: readonly string[] = [];
      let navigationPending = false;

      const sync = () => {
        if (navigationPending || marker(window.history.state)?.owner !== sessionOwner) return;
        const next = desired.current;
        if (samePath(currentPath, next)) return;
        let sharedDepth = 0;
        while (sharedDepth < Math.min(currentPath.length, next.length) &&
               currentPath[sharedDepth] === next[sharedDepth]) {
          sharedDepth += 1;
        }
        if (sharedDepth < currentPath.length) {
          // Rewind before a new branch so Forward cannot reopen a stale detail.
          navigationPending = true;
          window.history.go(sharedDepth - currentPath.length);
          return;
        }
        for (let level = currentPath.length + 1; level <= next.length; level += 1) {
          window.history.pushState(
            withMarker(window.history.state, sessionOwner, next.slice(0, level)),
            "",
          );
        }
        currentPath = [...next];
      };

      const onPop = (event: PopStateEvent) => {
        const reached = marker(event.state);
        if (reached?.owner !== sessionOwner) return;
        currentPath = reached.path;
        if (navigationPending) {
          navigationPending = false;
          sync();
        } else {
          navigate.current(reached.path);
        }
      };
      window.addEventListener("popstate", onPop);
      session.current = {
        sync,
        back: () => {
          if (currentPath.length > 0) window.history.back();
          else navigate.current([]);
        },
        dispose: () => {
          window.removeEventListener("popstate", onPop);
          retire(sessionOwner, baseState, navigationPending);
        },
      };
      sync();
    };
    const retiring = pendingRetirement;
    if (retiring) void retiring.then(initialize);
    else initialize();
    return () => {
      cancelled = true;
      session.current?.dispose();
      session.current = null;
    };
  }, []);

  const pathKey = path.join("\u0000");
  useEffect(() => session.current?.sync(), [pathKey]);

  return useCallback(() => {
    if (session.current) session.current.back();
    else navigate.current(desired.current.slice(0, -1));
  }, []);
}
