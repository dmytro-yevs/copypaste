import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

type ObserverCallback = (entries: ResizeObserverEntry[]) => void;

const { TestResizeObserver } = vi.hoisted(() => {
  class TestResizeObserver {
    static latest: TestResizeObserver | null = null;

    constructor(readonly callback: ObserverCallback) {
      TestResizeObserver.latest = this;
    }

    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}

    emit(entries: ResizeObserverEntry[]): void {
      this.callback(entries);
    }
  }
  return { TestResizeObserver };
});

vi.mock("@juggle/resize-observer", () => ({
  ResizeObserver: TestResizeObserver,
}));

import {
  ViewportMetricsProvider,
  useObservedElementSize,
  useViewportMetrics,
} from "./useViewportMetrics";

function metricsEntry(target: Element, width: number, height: number): ResizeObserverEntry {
  return {
    target,
    contentRect: { width, height } as DOMRectReadOnly,
  } as ResizeObserverEntry;
}

function Probe({ onRender }: { onRender: () => void }) {
  onRender();
  const viewport = useViewportMetrics();
  const observed = useObservedElementSize<HTMLDivElement>();
  return (
    <>
      <output data-testid="viewport">{`${viewport.width}x${viewport.height}`}</output>
      <div data-testid="observed" ref={observed.ref}>{`${observed.width}x${observed.height}`}</div>
    </>
  );
}

afterEach(() => {
  vi.restoreAllMocks();
  TestResizeObserver.latest = null;
});

describe("ViewportMetricsProvider", () => {
  it("coalesces matching observer entries and skips unchanged publications", () => {
    let scheduled: FrameRequestCallback | undefined;
    const frame = vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      scheduled = callback;
      return 1;
    });
    const renders = vi.fn();

    render(
      <ViewportMetricsProvider>
        <Probe onRender={renders} />
      </ViewportMetricsProvider>,
    );

    const observer = TestResizeObserver.latest;
    expect(observer).toBeTruthy();
    const rendersAfterMount = renders.mock.calls.length;
    const observed = screen.getByTestId("observed");
    const root = document.documentElement;

    act(() => {
      observer?.emit([
        metricsEntry(root, 800, 800),
        metricsEntry(root, 800, 800),
        metricsEntry(observed, 320, 48),
        metricsEntry(observed, 320, 48),
      ]);
      scheduled?.(0);
    });

    expect(frame).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("viewport").textContent).toBe("800x800");
    expect(screen.getByTestId("observed").textContent).toBe("320x48");
    expect(renders).toHaveBeenCalledTimes(rendersAfterMount + 1);
  });
});
