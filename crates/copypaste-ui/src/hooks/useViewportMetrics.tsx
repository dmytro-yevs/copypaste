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
import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";

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

interface ViewportMetrics extends ElementSize {}

type SizeSubscriber = (size: ElementSize) => void;

interface ViewportStoreState {
  metrics: ElementSize;
  pointer: PointerKind;
  sizeClass: SizeClass;
  setMetrics: (size: ElementSize) => void;
  setPointer: (pointer: PointerKind) => void;
}

interface ObservationContextValue {
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
function sizeClassFor(width: number): SizeClass {
  return width >= EXPANDED_MIN_PX ? "expanded" : "compact";
}

function createViewportStore(
  metrics: ElementSize,
  pointer: PointerKind = "fine",
): StoreApi<ViewportStoreState> {
  return createStore<ViewportStoreState>()((set) => ({
    metrics,
    pointer,
    sizeClass: sizeClassFor(metrics.width),
    setMetrics: (next) => set((current) => {
      if (sameSize(current.metrics, next)) return current;
      return {
        ...current,
        metrics: next,
        sizeClass: sizeClassFor(next.width),
      };
    }),
    setPointer: (next) => set((current) =>
      current.pointer === next ? current : { ...current, pointer: next }),
  }));
}

const FALLBACK_STORE = createViewportStore(initialSize);
const FALLBACK_OBSERVATION: ObservationContextValue = {
  observe: () => () => {},
};

const ViewportStoreContext = createContext<StoreApi<ViewportStoreState>>(FALLBACK_STORE);
const ObservationContext = createContext<ObservationContextValue>(FALLBACK_OBSERVATION);

export function ViewportMetricsProvider({ children }: { children: ReactNode }) {
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
  const storeRef = useRef<StoreApi<ViewportStoreState> | null>(null);
  storeRef.current ??= createViewportStore(
    windowSize(),
    pointerKind(pointerMedia, hoverMedia),
  );
  const store = storeRef.current;
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
        store.getState().setMetrics(size);
      }
      const subscribers = registry.get(element);
      if (!subscribers) continue;
      for (const subscriber of subscribers) subscriber(size);
    }
  }, [registry, store]);

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
    store.getState().setMetrics(windowSize());
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
  }, [observerRef, queueMeasurement, registry, store]);

  useEffect(() => {
    if (!pointerMedia && !hoverMedia) return;
    const update = () => store.getState().setPointer(pointerKind(pointerMedia, hoverMedia));
    update();
    pointerMedia?.addEventListener("change", update);
    hoverMedia?.addEventListener("change", update);
    return () => {
      pointerMedia?.removeEventListener("change", update);
      hoverMedia?.removeEventListener("change", update);
    };
  }, [hoverMedia, pointerMedia, store]);

  useLayoutEffect(() => {
    const root = document.documentElement;
    const apply = (pointer: PointerKind) => {
      root.dataset.pointer = pointer;
    };
    apply(store.getState().pointer);
    return store.subscribe((next, previous) => {
      if (next.pointer !== previous.pointer) apply(next.pointer);
    });
  }, [store]);

  const observation = useMemo<ObservationContextValue>(() => ({ observe }), [observe]);

  return (
    <ViewportStoreContext.Provider value={store}>
      <ObservationContext.Provider value={observation}>
        {children}
      </ObservationContext.Provider>
    </ViewportStoreContext.Provider>
  );
}

export function useViewportMetrics(): ViewportMetrics {
  const store = useContext(ViewportStoreContext);
  return useStore(store, (state) => state.metrics);
}

export function useViewportSizeClass(): SizeClass {
  const store = useContext(ViewportStoreContext);
  return useStore(store, (state) => state.sizeClass);
}

export function usePointerKind(): PointerKind {
  const store = useContext(ViewportStoreContext);
  return useStore(store, (state) => state.pointer);
}

export function useObservedElementSize<T extends Element>(): ElementSize & {
  ref: RefCallback<T>;
} {
  const { observe } = useContext(ObservationContext);
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
