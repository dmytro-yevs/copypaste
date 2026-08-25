import type { ReactNode } from "react";
import { DragDropProvider } from "@dnd-kit/react";
import { move } from "@dnd-kit/helpers";
import type { DragEndEvent } from "@dnd-kit/react";
import { useSortable } from "@dnd-kit/react/sortable";


type PinnedReorderProviderProps = {
  ids: readonly string[];
  onReorder: (ids: readonly string[]) => void;
  children: ReactNode;
};

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
