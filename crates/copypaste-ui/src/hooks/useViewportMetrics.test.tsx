import { act, render, screen } from "@testing-library/react";
import { StrictMode, useLayoutEffect, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

type ObserverCallback = (entries: ResizeObserverEntry[]) => void;
type TestMediaQuery = MediaQueryList & { setMatches: (matches: boolean) => void };

const maxTouchPointsDescriptor = Object.getOwnPropertyDescriptor(navigator, "maxTouchPoints");

const { TestResizeObserver } = vi.hoisted(() => {
  class TestResizeObserver {
    static instances: TestResizeObserver[] = [];
    static latest: TestResizeObserver | null = null;
    readonly observed: Element[] = [];
    readonly unobserved: Element[] = [];
    disconnects = 0;

    constructor(readonly callback: ObserverCallback) {
      TestResizeObserver.instances.push(this);
      TestResizeObserver.latest = this;
    }

    observe(element: Element): void {
      this.observed.push(element);
    }

    unobserve(element: Element): void {
      this.unobserved.push(element);
    }

    disconnect(): void {
      this.disconnects += 1;
    }

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

function setMaxTouchPoints(value: number): void {
  Object.defineProperty(navigator, "maxTouchPoints", { configurable: true, value });
}

function mediaQuery(query: string, matches: boolean): TestMediaQuery {
  const listeners = new Set<(event: MediaQueryListEvent) => void>();
  const list = {
    media: query,
    matches,
    onchange: null,
    addListener(listener: (event: MediaQueryListEvent) => void) {
      listeners.add(listener);
    },
    removeListener(listener: (event: MediaQueryListEvent) => void) {
      listeners.delete(listener);
    },
    addEventListener(_type: string, listener: (event: MediaQueryListEvent) => void) {
      listeners.add(listener);
    },
    removeEventListener(_type: string, listener: (event: MediaQueryListEvent) => void) {
      listeners.delete(listener);
    },
    dispatchEvent: () => false,
    setMatches(next: boolean) {
      if (next === list.matches) return;
      list.matches = next;
      for (const listener of listeners) {
        listener({ matches: next, media: query } as MediaQueryListEvent);
      }
    },
  };
  return list as unknown as TestMediaQuery;
}

function configurePointerMedia(coarse: boolean, noHover: boolean): {
  pointer: TestMediaQuery;
  hover: TestMediaQuery;
} {
  const pointer = mediaQuery("(pointer: coarse)", coarse);
  const hover = mediaQuery("(hover: none)", noHover);
  vi.spyOn(window, "matchMedia").mockImplementation((query) => {
    if (query === pointer.media) return pointer;
    if (query === hover.media) return hover;
    return mediaQuery(query, false);
  });
  return { pointer, hover };
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

function PointerProbe() {
  const { pointer } = useViewportMetrics();
  return <output data-testid="pointer">{pointer}</output>;
}

function ElementSubscriber({ element }: { element: HTMLDivElement }) {
  const { ref } = useObservedElementSize<HTMLDivElement>();
  useLayoutEffect(() => {
    ref(element);
    return () => {
      ref(null);
    };
  }, [element, ref]);
  return null;
}

function SharedElementProbe({ first, second }: { first: boolean; second: boolean }) {
  const [element, setElement] = useState<HTMLDivElement | null>(null);
  return (
    <>
      <div data-testid="shared" ref={setElement} />
      {element && first ? <ElementSubscriber element={element} /> : null}
      {element && second ? <ElementSubscriber element={element} /> : null}
    </>
  );
}

afterEach(() => {
  vi.restoreAllMocks();
  if (maxTouchPointsDescriptor) {
    Object.defineProperty(navigator, "maxTouchPoints", maxTouchPointsDescriptor);
  } else {
    delete (navigator as { maxTouchPoints?: number }).maxTouchPoints;
  }
  delete document.documentElement.dataset.pointer;
  TestResizeObserver.instances = [];
  TestResizeObserver.latest = null;
});

describe("ViewportMetricsProvider", () => {
  it("uses touch capability when a legacy WebView reports fine with no hover", () => {
    setMaxTouchPoints(5);
    const { hover } = configurePointerMedia(false, true);

    render(
      <ViewportMetricsProvider>
        <PointerProbe />
      </ViewportMetricsProvider>,
    );

    expect(screen.getByTestId("pointer").textContent).toBe("coarse");
    expect(document.documentElement.dataset.pointer).toBe("coarse");

    act(() => hover.setMatches(false));

    expect(screen.getByTestId("pointer").textContent).toBe("fine");
    expect(document.documentElement.dataset.pointer).toBe("fine");
  });

  it("keeps a fine pointer without a known touch capability", () => {
    setMaxTouchPoints(0);
    configurePointerMedia(false, true);
    const { unmount } = render(
      <ViewportMetricsProvider>
        <PointerProbe />
      </ViewportMetricsProvider>,
    );

    expect(screen.getByTestId("pointer").textContent).toBe("fine");
    unmount();

    setMaxTouchPoints(Number.NaN);
    configurePointerMedia(false, true);
    render(
      <ViewportMetricsProvider>
        <PointerProbe />
      </ViewportMetricsProvider>,
    );

    expect(screen.getByTestId("pointer").textContent).toBe("fine");
  });

  it("keeps modern coarse pointer detection authoritative", () => {
    setMaxTouchPoints(0);
    configurePointerMedia(true, false);

    render(
      <ViewportMetricsProvider>
        <PointerProbe />
      </ViewportMetricsProvider>,
    );

    expect(screen.getByTestId("pointer").textContent).toBe("coarse");
  });

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

  it("cleans up and re-establishes the shared observer under StrictMode", () => {
    const { unmount } = render(
      <StrictMode>
        <ViewportMetricsProvider>
          <Probe onRender={() => {}} />
        </ViewportMetricsProvider>
      </StrictMode>,
    );

    expect(TestResizeObserver.instances).toHaveLength(2);
    expect(TestResizeObserver.instances[0]?.disconnects).toBe(1);
    expect(TestResizeObserver.instances[1]?.observed).toContain(document.documentElement);

    unmount();

    expect(TestResizeObserver.instances[1]?.disconnects).toBe(1);
  });

  it("cancels pending measurements before unmount", () => {
    let scheduled: FrameRequestCallback | undefined;
    const cancel = vi.spyOn(window, "cancelAnimationFrame");
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      scheduled = callback;
      return 17;
    });
    const renders = vi.fn();
    const { unmount } = render(
      <ViewportMetricsProvider>
        <Probe onRender={renders} />
      </ViewportMetricsProvider>,
    );
    const observer = TestResizeObserver.latest;
    const observed = screen.getByTestId("observed");
    const rendersAfterMount = renders.mock.calls.length;

    act(() => observer?.emit([metricsEntry(observed, 320, 48)]));
    expect(scheduled).toBeTruthy();

    unmount();
    expect(cancel).toHaveBeenCalledWith(17);

    act(() => scheduled?.(0));
    expect(renders).toHaveBeenCalledTimes(rendersAfterMount);
  });

  it("retains an observed element until its final subscriber unmounts", () => {
    const { rerender } = render(
      <ViewportMetricsProvider>
        <SharedElementProbe first second />
      </ViewportMetricsProvider>,
    );
    const observer = TestResizeObserver.latest;
    const shared = screen.getByTestId("shared");

    expect(observer?.observed.filter((element) => element === shared)).toHaveLength(1);

    rerender(
      <ViewportMetricsProvider>
        <SharedElementProbe first second={false} />
      </ViewportMetricsProvider>,
    );
    expect(observer?.unobserved).not.toContain(shared);

    rerender(
      <ViewportMetricsProvider>
        <SharedElementProbe first={false} second={false} />
      </ViewportMetricsProvider>,
    );
    expect(observer?.unobserved).toContain(shared);
  });
});
