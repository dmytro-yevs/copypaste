/**
 * W-C6 / UIC-5 / VISM-11: Toast skin-token tests.
 *
 * Verifies:
 * 1. GlassToastItem no longer uses hardcoded `rounded-ide-lg` class (replaced by skin var).
 * 2. GlassToastItem no longer uses hardcoded `shadow-ide-sm` class (replaced by skin var).
 * 3. Toast bubble carries skin-driven border-radius via --skin-r-modal CSS var (UIC-5: modal tier).
 * 4. Toast bubble carries skin-driven box-shadow via --skin-shadow-card CSS var reference.
 * 5. surface-card material class is retained (correct floating-surface tier).
 * 6. All features preserved: auto-dismiss timer, dismiss button, kind variants (info/success/error/warning).
 * 7. UIC-5: border-radius uses --skin-r-modal (NOT --skin-r-card) — toast is a modal-tier surface.
 * 8. VISM-11: a leading semantic colour dot is rendered for each kind.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { GlassToast, ToastProvider, useToast, type ToastKind } from "./Toast";
import React from "react";

// ---------------------------------------------------------------------------
// §1  No hardcoded radius/shadow tailwind classes
// ---------------------------------------------------------------------------

describe("W-C6 §1 — GlassToastItem: no hardcoded radius or shadow Tailwind class", () => {
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
// §2  Skin-driven radius and shadow via CSS var
// ---------------------------------------------------------------------------

describe("W-C6 / UIC-5 §2 — GlassToastItem: skin-driven radius and shadow", () => {
  it("UIC-5: bubble has border-radius referencing --skin-r-modal (NOT --skin-r-card)", () => {
    const { container } = render(
      <GlassToast msg={{ id: "t1", text: "hello" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]') as HTMLElement | null;
    expect(bubble).not.toBeNull();

    // Must reference --skin-r-modal (modal-tier radius), not --skin-r-card.
    const inlineStyle = bubble!.style.borderRadius;
    const hasModalVar = inlineStyle.includes("--skin-r-modal") || bubble!.className.includes("--skin-r-modal");
    expect(hasModalVar).toBe(true);

    // Must NOT reference --skin-r-card (card radius is wrong tier for a floating toast).
    const hasCardVar = inlineStyle.includes("--skin-r-card") || bubble!.className.includes("--skin-r-card");
    expect(hasCardVar).toBe(false);
  });

  it("bubble has box-shadow referencing --skin-shadow-card via inline style or arbitrary class", () => {
    const { container } = render(
      <GlassToast msg={{ id: "t1", text: "hello" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]') as HTMLElement | null;
    expect(bubble).not.toBeNull();

    const inlineStyle = bubble!.style.boxShadow;
    const hasVarInStyle = inlineStyle.includes("--skin-shadow-card");
    const hasVarInClass = bubble!.className.includes("--skin-shadow-card");

    expect(hasVarInStyle || hasVarInClass).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// §3  Material class preserved
// ---------------------------------------------------------------------------

describe("W-C6 §3 — GlassToastItem: surface-card material retained", () => {
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

describe("W-C6 §4 — feature preservation: auto-dismiss", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("toast dismisses after default 3000ms", () => {
    const onDismiss = vi.fn();
    render(
      <GlassToast msg={{ id: "t42", text: "bye" }} onDismiss={onDismiss} />,
    );
    expect(onDismiss).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(onDismiss).toHaveBeenCalledWith("t42");
  });

  it("toast respects custom duration", () => {
    const onDismiss = vi.fn();
    render(
      <GlassToast
        msg={{ id: "t43", text: "short", duration: 1000 }}
        onDismiss={onDismiss}
      />,
    );
    act(() => {
      vi.advanceTimersByTime(999);
    });
    expect(onDismiss).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(onDismiss).toHaveBeenCalledWith("t43");
  });
});

// ---------------------------------------------------------------------------
// §5  Feature preservation: dismiss button
// ---------------------------------------------------------------------------

describe("W-C6 §5 — feature preservation: dismiss button", () => {
  it("dismiss button calls onDismiss with the message id", () => {
    const onDismiss = vi.fn();
    render(
      <GlassToast msg={{ id: "t99", text: "click me" }} onDismiss={onDismiss} />,
    );
    const btn = screen.getByRole("button", { name: /dismiss/i });
    fireEvent.click(btn);
    expect(onDismiss).toHaveBeenCalledWith("t99");
  });
});

// ---------------------------------------------------------------------------
// §6  Feature preservation: kind variants produce correct colour classes
// ---------------------------------------------------------------------------

describe("W-C6 §6 — feature preservation: kind variants", () => {
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

describe("VISM-11 §7 — GlassToastItem: leading semantic colour dot", () => {
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
      // The dot is an aria-hidden span with h-2 w-2 rounded-full and the kind colour class.
      const dots = container.querySelectorAll('[aria-hidden="true"]');
      const dot = Array.from(dots).find((el) =>
        el.className.includes("h-2") && el.className.includes("w-2") && el.className.includes("rounded-full")
      );
      expect(dot).not.toBeUndefined();
      expect(dot!.className).toMatch(new RegExp(dotCls));
    });
  }

  it("dot is aria-hidden (purely presentational — kind text colour conveys the same info)", () => {
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
// §8  ToastProvider integration: show + auto-dismiss via provider
// ---------------------------------------------------------------------------

function TestConsumer() {
  const toast = useToast();
  return (
    <button onClick={() => toast.show("provider toast", { kind: "success" })}>
      show
    </button>
  );
}

describe("W-C6 §8 — ToastProvider integration", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("ToastProvider renders toast and dismisses it after 3s", () => {
    render(
      <ToastProvider>
        <TestConsumer />
      </ToastProvider>,
    );
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: /show/i }));
    });
    expect(screen.getByText("provider toast")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(screen.queryByText("provider toast")).toBeNull();
  });
});
