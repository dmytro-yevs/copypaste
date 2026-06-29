/**
 * Phase 4: Toast uses fixed design tokens (--r-card, --sh1).
 * Updated from W-C6/UIC-5: old skin vars replaced with fixed tokens.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { GlassToast, ToastProvider, useToast, type ToastKind } from "./Toast";
import React from "react";

// ---------------------------------------------------------------------------
// §1  No hardcoded radius/shadow tailwind classes
// ---------------------------------------------------------------------------

describe("Toast §1 — no hardcoded radius or shadow Tailwind class", () => {
  it("does NOT use rounded-ide-lg (hardcoded 14px radius)", () => {
    const { container } = render(
      <GlassToast msg={{ id: "t1", text: "hello" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]');
    expect(bubble).not.toBeNull();
    expect(bubble!.className).not.toMatch(/rounded-ide-lg/);
  });

  it("does NOT use shadow-ide-sm (hardcoded e2 shadow)", () => {
    const { container } = render(
      <GlassToast msg={{ id: "t1", text: "hello" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]');
    expect(bubble).not.toBeNull();
    expect(bubble!.className).not.toMatch(/shadow-ide-sm/);
  });
});

// ---------------------------------------------------------------------------
// §2  Fixed design tokens: --r-card radius, --sh1 shadow
// ---------------------------------------------------------------------------

describe("Toast §2 — fixed design tokens (Phase 4)", () => {
  it("bubble has border-radius using var(--r-card)", () => {
    const { container } = render(
      <GlassToast msg={{ id: "t1", text: "hello" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]') as HTMLElement | null;
    expect(bubble).not.toBeNull();
    expect(bubble!.style.borderRadius).toBe("var(--r-card)");
  });

  it("bubble has box-shadow using var(--sh1)", () => {
    const { container } = render(
      <GlassToast msg={{ id: "t1", text: "hello" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]') as HTMLElement | null;
    expect(bubble).not.toBeNull();
    expect(bubble!.style.boxShadow).toBe("var(--sh1)");
  });
});

// ---------------------------------------------------------------------------
// §3  Material class preserved
// ---------------------------------------------------------------------------

describe("Toast §3 — surface-card material retained", () => {
  it("bubble still carries surface-card class for glass material", () => {
    const { container } = render(
      <GlassToast msg={{ id: "t1", text: "hello" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]');
    expect(bubble!.className).toMatch(/surface-card/);
  });
});

// ---------------------------------------------------------------------------
// §4  Feature preservation: auto-dismiss
// ---------------------------------------------------------------------------

describe("Toast §4 — feature preservation: auto-dismiss", () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it("toast dismisses after default 3000ms", () => {
    const onDismiss = vi.fn();
    render(<GlassToast msg={{ id: "t42", text: "bye" }} onDismiss={onDismiss} />);
    expect(onDismiss).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(3000); });
    expect(onDismiss).toHaveBeenCalledWith("t42");
  });

  it("toast respects custom duration", () => {
    const onDismiss = vi.fn();
    render(
      <GlassToast msg={{ id: "t43", text: "short", duration: 1000 }} onDismiss={onDismiss} />,
    );
    act(() => { vi.advanceTimersByTime(999); });
    expect(onDismiss).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(1); });
    expect(onDismiss).toHaveBeenCalledWith("t43");
  });
});

// ---------------------------------------------------------------------------
// §5  Feature preservation: dismiss button
// ---------------------------------------------------------------------------

describe("Toast §5 — feature preservation: dismiss button", () => {
  it("dismiss button calls onDismiss with the message id", () => {
    const onDismiss = vi.fn();
    render(<GlassToast msg={{ id: "t99", text: "click me" }} onDismiss={onDismiss} />);
    const btn = screen.getByRole("button", { name: /dismiss/i });
    fireEvent.click(btn);
    expect(onDismiss).toHaveBeenCalledWith("t99");
  });
});

// ---------------------------------------------------------------------------
// §6  Feature preservation: kind variants
// ---------------------------------------------------------------------------

describe("Toast §6 — feature preservation: kind variants", () => {
  const cases: { kind: ToastKind; cls: string }[] = [
    { kind: "info",    cls: "text-ide-text" },
    { kind: "success", cls: "text-ide-success" },
    { kind: "error",   cls: "text-ide-danger" },
    { kind: "warning", cls: "text-ide-warning" },
  ];

  for (const { kind, cls } of cases) {
    it(`kind="${kind}" applies ${cls}`, () => {
      const { container } = render(
        <GlassToast msg={{ id: "kv", text: "msg", kind }} onDismiss={() => {}} />,
      );
      const bubble = container.querySelector('[role="status"]');
      expect(bubble!.className).toMatch(new RegExp(cls));
    });
  }
});

// ---------------------------------------------------------------------------
// §7  VISM-11: semantic colour dot
// ---------------------------------------------------------------------------

describe("Toast VISM-11 §7 — leading semantic colour dot", () => {
  const dotCases: { kind: ToastKind; dotCls: string }[] = [
    { kind: "info",    dotCls: "bg-ide-text" },
    { kind: "success", dotCls: "bg-ide-success" },
    { kind: "error",   dotCls: "bg-ide-danger" },
    { kind: "warning", dotCls: "bg-ide-warning" },
  ];

  for (const { kind, dotCls } of dotCases) {
    it(`kind="${kind}" renders a leading dot with class ${dotCls}`, () => {
      const { container } = render(
        <GlassToast msg={{ id: "dot-test", text: "msg", kind }} onDismiss={() => {}} />,
      );
      const dots = container.querySelectorAll('[aria-hidden="true"]');
      const dot = Array.from(dots).find((el) =>
        el.className.includes("h-2") && el.className.includes("w-2") && el.className.includes("rounded-full")
      );
      expect(dot).not.toBeUndefined();
      expect(dot!.className).toMatch(new RegExp(dotCls));
    });
  }

  it("dot is aria-hidden", () => {
    const { container } = render(
      <GlassToast msg={{ id: "a11y-dot", text: "msg", kind: "error" }} onDismiss={() => {}} />,
    );
    const dots = container.querySelectorAll('[aria-hidden="true"]');
    const dot = Array.from(dots).find((el) =>
      el.className.includes("h-2") && el.className.includes("rounded-full")
    );
    expect(dot).not.toBeUndefined();
    expect(dot!.getAttribute("aria-hidden")).toBe("true");
  });
});

// ---------------------------------------------------------------------------
// §8  ToastProvider integration
// ---------------------------------------------------------------------------

function TestConsumer() {
  const toast = useToast();
  return (
    <button onClick={() => toast.show("provider toast", { kind: "success" })}>
      show
    </button>
  );
}

describe("Toast §8 — ToastProvider integration", () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it("ToastProvider renders toast and dismisses it after 3s", () => {
    render(
      <ToastProvider>
        <TestConsumer />
      </ToastProvider>,
    );
    act(() => { fireEvent.click(screen.getByRole("button", { name: /show/i })); });
    expect(screen.getByText("provider toast")).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(3000); });
    expect(screen.queryByText("provider toast")).toBeNull();
  });
});
