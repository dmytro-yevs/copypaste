import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Dialog, type DialogProps } from "./Dialog";
import { __resetScrollLockForTests, scrollLockDepth } from "./scrollLock";

afterEach(() => {
  cleanup();
  __resetScrollLockForTests();
});

function Harness(props: Partial<DialogProps>) {
  return (
    <Dialog labelledBy="t" onClose={props.onClose ?? (() => {})} {...props}>
      <h2 id="t">Title</h2>
      <button>First</button>
      <button>Second</button>
    </Dialog>
  );
}

describe("Dialog — structure & ARIA", () => {
  it("renders a portaled role=dialog with aria-modal + aria-labelledby", () => {
    render(<Harness />);
    const dlg = screen.getByRole("dialog");
    expect(dlg).toHaveAttribute("aria-modal", "true");
    expect(dlg).toHaveAttribute("aria-labelledby", "t");
    expect(document.body.contains(dlg)).toBe(true);
  });
});

describe("Dialog — focus", () => {
  it("moves initial focus to the first focusable descendant", () => {
    render(<Harness />);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "First" }));
  });

  it("falls back to the container when there is no focusable descendant", () => {
    render(
      <Dialog labelledBy="t" onClose={() => {}}>
        <h2 id="t">No focusables here</h2>
      </Dialog>,
    );
    const dlg = screen.getByRole("dialog");
    expect(document.activeElement).toBe(dlg);
    expect(dlg).toHaveAttribute("tabindex", "-1");
  });

  it("Tab from the last focusable wraps to the first", () => {
    render(<Harness />);
    const first = screen.getByRole("button", { name: "First" });
    screen.getByRole("button", { name: "Second" }).focus();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Tab" });
    expect(document.activeElement).toBe(first);
  });

  it("Shift+Tab from the first wraps to the last", () => {
    render(<Harness />);
    const second = screen.getByRole("button", { name: "Second" });
    screen.getByRole("button", { name: "First" }).focus();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(second);
  });

  it("restores focus to the trigger element on close", () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();
    const { unmount } = render(<Harness />);
    expect(document.activeElement).not.toBe(trigger);
    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });
});

describe("Dialog — dismissal", () => {
  it("Escape calls onClose (default)", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("Escape is inert when dismissOnEscape=false", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} dismissOnEscape={false} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });

  it("backdrop click dismisses, panel click does not", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);
    const dlg = screen.getByRole("dialog");
    fireEvent.click(dlg); // inside the panel → stopPropagation, no dismiss
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(dlg.parentElement as HTMLElement); // the scrim backdrop
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("backdrop click is inert when dismissOnBackdrop=false", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} dismissOnBackdrop={false} />);
    fireEvent.click(screen.getByRole("dialog").parentElement as HTMLElement);
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe("Dialog — reference-counted scroll-lock", () => {
  it("locks body scroll while open and restores on close", () => {
    expect(document.body.style.overflow).toBe("");
    const { unmount } = render(<Harness />);
    expect(document.body.style.overflow).toBe("hidden");
    unmount();
    expect(document.body.style.overflow).toBe("");
  });

  it("stacked dialogs: closing the inner one keeps scroll locked until the outer closes", () => {
    const outer = render(<Harness />);
    expect(document.body.style.overflow).toBe("hidden");
    const inner = render(<Harness />);
    expect(scrollLockDepth()).toBe(2);

    inner.unmount();
    expect(document.body.style.overflow).toBe("hidden"); // still locked by outer
    expect(scrollLockDepth()).toBe(1);

    outer.unmount();
    expect(document.body.style.overflow).toBe("");
    expect(scrollLockDepth()).toBe(0);
  });
});
