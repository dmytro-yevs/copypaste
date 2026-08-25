import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
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
  const [pointer, setPointer] = useState<PointerKind>(() =>
    pointerMedia?.matches ? "coarse" : "fine",
  );
  const [registry] = useState(() => new Map<Element, Set<SizeSubscriber>>());
  const [observerRef] = useState<{
    current: MaintainedResizeObserver | null;
  }>(() => ({ current: null }));

  const observe = useCallback((element: Element, subscriber: SizeSubscriber) => {
    let subscribers = registry.get(element);
    if (!subscribers) {
      subscribers = new Set();
      registry.set(element, subscribers);
      observerRef.current?.observe(element);
    }
    subscribers.add(subscriber);
    const bounds = element.getBoundingClientRect();
    subscriber({ width: bounds.width, height: bounds.height });
    return () => {
      const current = registry.get(element);
      current?.delete(subscriber);
      if (current?.size === 0) {
        registry.delete(element);
        observerRef.current?.unobserve(element);
      }
    };
  }, [observerRef, registry]);

  useLayoutEffect(() => {
    const root = document.documentElement;
    const observer = new MaintainedResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.target === root) {
          setViewport({
            width: root.clientWidth || entry.contentRect.width,
            height: root.clientHeight || entry.contentRect.height,
          });
        }
        const subscribers = registry.get(entry.target);
        if (!subscribers) continue;
        const size = {
          width: entry.contentRect.width,
          height: entry.contentRect.height,
        };
        for (const subscriber of subscribers) subscriber(size);
      }
    });
    observerRef.current = observer;
    observer.observe(root);
    for (const element of registry.keys()) observer.observe(element);
    setViewport(windowSize());
    return () => {
      observerRef.current = null;
      observer.disconnect();
    };
  }, [observerRef, registry]);

  useEffect(() => {
    if (!pointerMedia) return;
    const update = () => setPointer(pointerMedia.matches ? "coarse" : "fine");
    update();
    pointerMedia.addEventListener("change", update);
    return () => pointerMedia.removeEventListener("change", update);
  }, [pointerMedia]);

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
