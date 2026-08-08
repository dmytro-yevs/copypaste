import type { ReactNode } from "react";
import { DragDropProvider } from "@dnd-kit/react";
import { arrayMove, move } from "@dnd-kit/helpers";
import type { DragEndEvent } from "@dnd-kit/react";
import { useSortable } from "@dnd-kit/react/sortable";

import type { Item } from "@/lib/ipc";

interface PinnedReorderProviderProps {
  ids: readonly string[];
  onReorder: (ids: readonly string[]) => void;
  children: ReactNode;
}

export function PinnedReorderProvider({
  ids,
  onReorder,
  children,
}: PinnedReorderProviderProps) {
  function finish(event: DragEndEvent) {
    const reordered = droppedPinnedOrder(ids, event);
    if (reordered) onReorder(reordered);
  }

  return (
    <DragDropProvider onDragEnd={finish}>
      {children}
    </DragDropProvider>
  );
}

export function droppedPinnedOrder(
  ids: readonly string[],
  event: DragEndEvent,
): readonly string[] | null {
  if (event.canceled) return null;
  const reordered = move([...ids], event);
  return reordered.some((id, index) => id !== ids[index]) ? reordered : null;
}

export interface SortableBindings {
  elementRef: (element: Element | null) => void;
  handleRef: (element: Element | null) => void;
  dragging: boolean;
  dropTarget: boolean;
}

interface SortablePinnedProps {
  id: string;
  index: number;
  children: (bindings: SortableBindings) => ReactNode;
}

export function SortablePinned({ id, index, children }: SortablePinnedProps) {
  const { ref, handleRef, isDragging, isDropTarget } = useSortable({
    id,
    index,
    group: "pinned-history",
    type: "pinned-history",
    accept: "pinned-history",
  });

  return children({
    elementRef: ref,
    handleRef,
    dragging: isDragging,
    dropTarget: isDropTarget,
  });
}

export function keyboardPinnedOrder(
  items: readonly Item[],
  activeId: string,
  direction: -1 | 1,
): readonly string[] | null {
  const pinned = items.filter((item) => item.pinned);
  const from = pinned.findIndex((item) => item.id === activeId);
  const to = from + direction;
  if (from < 0 || to < 0 || to >= pinned.length) return null;
  return arrayMove(pinned, from, to).map((item) => item.id);
}
