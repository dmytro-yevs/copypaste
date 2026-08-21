import { describe, expect, it } from "vitest";

import {
  modalDescriptionClass,
  modalFooterClass,
  modalFrameVariants,
  modalHeaderClass,
  modalOverlayClass,
  modalTitleClass,
} from "@/components/ui/modal-shell";

describe("the shared modal shell", () => {
  it("owns the complete modal frame and section style system", () => {
    const frame = modalFrameVariants({ presentation: "modal" });

    expect(modalOverlayClass).toContain("bg-scrim");
    expect(frame).toContain("border-border");
    expect(frame).toContain("bg-card");
    expect(frame).toContain("shadow-3");
    expect(modalHeaderClass).toContain("text-left");
    expect(modalFooterClass).toContain("flex-col-reverse");
    expect(modalTitleClass).toContain("font-semibold");
    expect(modalDescriptionClass).toContain("text-muted-foreground");
  });

  it("provides the Android sheet geometry without call-site chrome", () => {
    const sheet = modalFrameVariants({ presentation: "sheet" });

    expect(sheet).toContain("top-auto");
    expect(sheet).toContain("bottom-[calc(var(--inset-bottom)+var(--s-2))]");
    expect(sheet).toContain("max-w-none");
    expect(sheet).toContain("rounded-xl");
    expect(sheet).toContain("p-4");
  });
});
