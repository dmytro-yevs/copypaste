import { describe, expect, it } from "vitest";
import {
  isLoadingState,
  isErrorState,
  isReadyState,
  isOfflineState,
  isNotReadyState,
  isDegradedState,
  type LoadState,
} from "./loadState";

describe("loadState type guards", () => {
  const allStates: LoadState[] = [
    "loading",
    "ready",
    "offline",
    "not_ready",
    "degraded",
    "error",
  ];

  it("isLoadingState returns true only for 'loading'", () => {
    expect(isLoadingState("loading")).toBe(true);
    for (const s of allStates.filter((s) => s !== "loading")) {
      expect(isLoadingState(s)).toBe(false);
    }
  });

  it("isReadyState returns true only for 'ready'", () => {
    expect(isReadyState("ready")).toBe(true);
    for (const s of allStates.filter((s) => s !== "ready")) {
      expect(isReadyState(s)).toBe(false);
    }
  });

  it("isOfflineState returns true only for 'offline'", () => {
    expect(isOfflineState("offline")).toBe(true);
    for (const s of allStates.filter((s) => s !== "offline")) {
      expect(isOfflineState(s)).toBe(false);
    }
  });

  it("isNotReadyState returns true only for 'not_ready'", () => {
    expect(isNotReadyState("not_ready")).toBe(true);
    for (const s of allStates.filter((s) => s !== "not_ready")) {
      expect(isNotReadyState(s)).toBe(false);
    }
  });

  it("isDegradedState returns true only for 'degraded'", () => {
    expect(isDegradedState("degraded")).toBe(true);
    for (const s of allStates.filter((s) => s !== "degraded")) {
      expect(isDegradedState(s)).toBe(false);
    }
  });

  it("isErrorState returns true only for 'error'", () => {
    expect(isErrorState("error")).toBe(true);
    for (const s of allStates.filter((s) => s !== "error")) {
      expect(isErrorState(s)).toBe(false);
    }
  });

  it("isLoadingState rejects non-string inputs", () => {
    expect(isLoadingState(null as unknown as LoadState)).toBe(false);
    expect(isLoadingState(undefined as unknown as LoadState)).toBe(false);
    expect(isLoadingState(42 as unknown as LoadState)).toBe(false);
  });
});
