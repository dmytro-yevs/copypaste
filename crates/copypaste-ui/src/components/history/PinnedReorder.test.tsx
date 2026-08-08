import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import {
  droppedPinnedOrder,
  keyboardPinnedOrder,
  PinnedReorderProvider,
  SortablePinned,
} from "@/components/history/PinnedReorder";
import { items } from "@/test/harness";

function rect(top: number): DOMRect {
  return {
    x: 0,
    y: top,
    top,
    left: 0,
    right: 300,
    bottom: top + 60,
    width: 300,
    height: 60,
    toJSON: () => ({}),
  } as DOMRect;
}

function setup(onReorder = vi.fn()) {
  const ids = ["one", "two", "three"];
  render(
    <PinnedReorderProvider ids={ids} onReorder={onReorder}>
      {ids.map((id, index) => (
        <SortablePinned key={id} id={id} index={index}>
          {({ elementRef, handleRef }) => (
            <div
              ref={(element) => {
                if (element) element.getBoundingClientRect = () => rect(index * 60);
                elementRef(element);
              }}
              data-testid={`sortable-${id}`}
            >
              <button ref={handleRef} data-testid={`handle-${id}`}>
                {id}
              </button>
            </div>
          )}
        </SortablePinned>
      ))}
    </PinnedReorderProvider>,
  );
  return { ids, onReorder };
}

async function startKeyboardDrag() {
  const handle = screen.getByTestId("handle-two");
  handle.focus();
  fireEvent.keyDown(handle, { key: "Enter", code: "Enter" });
  await waitFor(() =>
    expect(
      screen
        .getAllByTestId("sortable-two")
        .some((element) => element.hasAttribute("data-dnd-dragging")),
    ).toBe(true),
  );
  return handle;
}

describe("pinned drag reorder", () => {
  it.each(["mouse", "touch"])("activates a %s pointer drag", async (pointerType) => {
    const { onReorder } = setup();
    const handle = screen.getByTestId("handle-two");
    await waitFor(() =>
      expect(handle.getAttribute("aria-roledescription")).toBe("draggable"),
    );
    fireEvent.pointerDown(handle, {
      pointerId: 1,
      pointerType,
      isPrimary: true,
      button: 0,
      clientX: 20,
      clientY: 80,
    });
    if (pointerType === "touch") {
      await new Promise((resolve) => setTimeout(resolve, 275));
    }
    await waitFor(() =>
      expect(
        screen
          .getAllByTestId("sortable-two")
          .some((element) => element.hasAttribute("data-dnd-dragging")),
      ).toBe(true),
    );
    fireEvent.pointerCancel(document, {
      pointerId: 1,
      pointerType,
      isPrimary: true,
      button: 0,
    });
    await waitFor(() =>
      expect(handle.getAttribute("aria-grabbed")).toBe("false"),
    );
    expect(onReorder).not.toHaveBeenCalled();
  });

  it("commits the complete order from a pointer drop", () => {
    const event = {
      operation: {
        source: { id: "two" },
        target: { id: "one" },
        canceled: false,
      },
    } as Parameters<typeof droppedPinnedOrder>[1];

    expect(droppedPinnedOrder(["one", "two", "three"], event)).toEqual([
      "two",
      "one",
      "three",
    ]);
  });

  it("matches the complete pointer order from the keyboard", async () => {
    const { onReorder } = setup();
    await startKeyboardDrag();
    fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
    await waitFor(() =>
      expect(screen.getAllByTestId(/^sortable-/)[0]?.textContent).toBe("two"),
    );
    fireEvent.keyDown(document, { key: "Enter", code: "Enter" });

    await waitFor(() =>
      expect(onReorder).toHaveBeenCalledWith(["two", "one", "three"]),
    );
  });

  it("restores the order and sends no IPC update when canceled", async () => {
    const { onReorder } = setup();
    await startKeyboardDrag();
    fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
    await waitFor(() =>
      expect(screen.getAllByTestId(/^sortable-/)[0]?.textContent).toBe("two"),
    );
    fireEvent.keyDown(document, { key: "Escape", code: "Escape" });

    await waitFor(() =>
      expect(screen.getAllByTestId(/^sortable-/)[0]?.textContent).toBe("one"),
    );
    expect(onReorder).not.toHaveBeenCalled();
  });
});

describe("keyboardPinnedOrder", () => {
  it("moves only within pins and returns the same complete order as drag", () => {
    const data = items(4).map((item, index) => ({
      ...item,
      pinned: index < 3,
    }));
    expect(keyboardPinnedOrder(data, data[1]!.id, -1)).toEqual([
      data[1]!.id,
      data[0]!.id,
      data[2]!.id,
    ]);
    expect(keyboardPinnedOrder(data, data[3]!.id, -1)).toBeNull();
  });
});
