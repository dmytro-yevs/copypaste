import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type RefCallback,
  type ReactNode,
} from "react";
import { ResizeObserver as MaintainedResizeObserver } from "@juggle/resize-observer";

import { EXPANDED_MIN_PX } from "@/lib/layoutBreakpoints";

export type PointerKind = "coarse" | "fine";
export type SizeClass = "compact" | "expanded";

interface ElementSize {
  width: number;
  height: number;
}

function sameSize(left: ElementSize, right: ElementSize): boolean {
  return left.width === right.width && left.height === right.height;
}

interface ViewportMetrics extends ElementSize {
  pointer: PointerKind;
  sizeClass: SizeClass;
}

type SizeSubscriber = (size: ElementSize) => void;

interface ViewportContextValue extends ViewportMetrics {
  observe: (element: Element, subscriber: SizeSubscriber) => () => void;
}

function windowSize(): ElementSize {
  if (typeof window === "undefined") return { width: 0, height: 0 };
  return {
    width: document.documentElement.clientWidth || window.innerWidth,
    height: document.documentElement.clientHeight || window.innerHeight,
  };
}

function hasTouchCapability(): boolean {
  if (typeof navigator === "undefined") return false;
  return Number.isFinite(navigator.maxTouchPoints) && navigator.maxTouchPoints > 0;
}

function pointerKind(
  pointerMedia: MediaQueryList | null,
  hoverMedia: MediaQueryList | null,
): PointerKind {
  if (pointerMedia?.matches) return "coarse";
  return hasTouchCapability() && hoverMedia?.matches ? "coarse" : "fine";
}

const initialSize = windowSize();
const FALLBACK: ViewportContextValue = {
  ...initialSize,
  pointer: "fine",
  sizeClass: initialSize.width >= EXPANDED_MIN_PX ? "expanded" : "compact",
  observe: () => () => {},
};

const ViewportContext = createContext<ViewportContextValue>(FALLBACK);

export function ViewportMetricsProvider({ children }: { children: ReactNode }) {
  const [viewport, setViewport] = useState(windowSize);
  const [pointerMedia] = useState(() =>
    typeof window !== "undefined" && window.matchMedia
      ? window.matchMedia("(pointer: coarse)")
      : null,
  );
  const [hoverMedia] = useState(() =>
    typeof window !== "undefined" && window.matchMedia
      ? window.matchMedia("(hover: none)")
      : null,
  );
  const [pointer, setPointer] = useState<PointerKind>(() =>
    pointerKind(pointerMedia, hoverMedia),
  );
  const [registry] = useState(() => new Map<Element, Set<SizeSubscriber>>());
  const [observerRef] = useState<{
    current: MaintainedResizeObserver | null;
  }>(() => ({ current: null }));
  const pendingMeasurements = useRef(new Map<Element, ElementSize>());
  const publishedMeasurements = useRef(new Map<Element, ElementSize>());
  const frame = useRef<number | null>(null);

  const flushMeasurements = useCallback(() => {
    frame.current = null;
    const measurements = [...pendingMeasurements.current];
    pendingMeasurements.current.clear();
    for (const [element, size] of measurements) {
      const previous = publishedMeasurements.current.get(element);
      if (previous && sameSize(previous, size)) continue;
      publishedMeasurements.current.set(element, size);
      if (element === document.documentElement) {
        setViewport((current) => (sameSize(current, size) ? current : size));
      }
      const subscribers = registry.get(element);
      if (!subscribers) continue;
      for (const subscriber of subscribers) subscriber(size);
    }
  }, [registry]);

  const queueMeasurement = useCallback((element: Element, size: ElementSize) => {
    const pending = pendingMeasurements.current.get(element);
    if (pending && sameSize(pending, size)) return;
    pendingMeasurements.current.set(element, size);
    if (frame.current !== null) return;
    frame.current = window.requestAnimationFrame(flushMeasurements);
  }, [flushMeasurements]);

  const observe = useCallback((element: Element, subscriber: SizeSubscriber) => {
    let subscribers = registry.get(element);
    if (!subscribers) {
      subscribers = new Set();
      registry.set(element, subscribers);
      observerRef.current?.observe(element);
    }
    subscribers.add(subscriber);
    const bounds = element.getBoundingClientRect();
    const size = { width: bounds.width, height: bounds.height };
    publishedMeasurements.current.set(element, size);
    subscriber(size);
    return () => {
      const current = registry.get(element);
      current?.delete(subscriber);
      if (current?.size === 0) {
        registry.delete(element);
        pendingMeasurements.current.delete(element);
        publishedMeasurements.current.delete(element);
        observerRef.current?.unobserve(element);
      }
    };
  }, [observerRef, registry]);

  useLayoutEffect(() => {
    const root = document.documentElement;
    const observer = new MaintainedResizeObserver((entries) => {
      for (const entry of entries) {
        const size = entry.target === root
          ? {
              width: root.clientWidth || entry.contentRect.width,
              height: root.clientHeight || entry.contentRect.height,
            }
          : {
              width: entry.contentRect.width,
              height: entry.contentRect.height,
            };
        queueMeasurement(entry.target, size);
      }
    });
    observerRef.current = observer;
    observer.observe(root);
    for (const element of registry.keys()) observer.observe(element);
    setViewport((current) => {
      const size = windowSize();
      return sameSize(current, size) ? current : size;
    });
    return () => {
      observerRef.current = null;
      observer.disconnect();
      if (frame.current !== null) {
        window.cancelAnimationFrame(frame.current);
        frame.current = null;
      }
      pendingMeasurements.current.clear();
      publishedMeasurements.current.clear();
    };
  }, [observerRef, queueMeasurement, registry]);

  useEffect(() => {
    if (!pointerMedia && !hoverMedia) return;
    const update = () => setPointer(pointerKind(pointerMedia, hoverMedia));
    update();
    pointerMedia?.addEventListener("change", update);
    hoverMedia?.addEventListener("change", update);
    return () => {
      pointerMedia?.removeEventListener("change", update);
      hoverMedia?.removeEventListener("change", update);
    };
  }, [hoverMedia, pointerMedia]);

  useLayoutEffect(() => {
    document.documentElement.dataset.pointer = pointer;
  }, [pointer]);

  const value = useMemo<ViewportContextValue>(
    () => ({
      ...viewport,
      pointer,
      sizeClass: viewport.width >= EXPANDED_MIN_PX ? "expanded" : "compact",
      observe,
    }),
    [observe, pointer, viewport],
  );

  return (
    <ViewportContext.Provider value={value}>
      {children}
    </ViewportContext.Provider>
  );
}

export function useViewportMetrics(): ViewportMetrics {
  const { observe: _observe, ...metrics } = useContext(ViewportContext);
  return metrics;
}

export function useObservedElementSize<T extends Element>(): ElementSize & {
  ref: RefCallback<T>;
} {
  const { observe } = useContext(ViewportContext);
  const [element, setElement] = useState<T | null>(null);
  const [size, setSize] = useState<ElementSize>({ width: 0, height: 0 });
  const ref = useCallback<RefCallback<T>>((node) => setElement(node), []);
  const update = useCallback((next: ElementSize) => {
    setSize((current) =>
      current.width === next.width && current.height === next.height ? current : next,
    );
  }, []);

  useLayoutEffect(() => {
    if (!element) return;
    return observe(element, update);
  }, [element, observe, update]);

  return { ...size, ref };
}
