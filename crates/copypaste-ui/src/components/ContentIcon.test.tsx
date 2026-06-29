/**
 * ContentIcon + KindChip shared component tests (CopyPaste-tsb).
 *
 * Verifies:
 *  1. ContentIcon renders the right aria-label / class for each content type.
 *  2. KindChip renders the correct label and color class.
 *  3. density pref round-trips through the store.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { ContentIcon, KindChip, kindFallback } from "./ContentIcon";
import { useUI } from "../store";
import { act } from "react";

// ---------------------------------------------------------------------------
// ContentIcon
// ---------------------------------------------------------------------------

describe("ContentIcon", () => {
  it("renders text icon with faint class for 'text'", () => {
    const { container } = render(<ContentIcon contentType="text" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // 5917.80: the text glyph carries text-ide-faint (grey), not accent (blue)
    expect(svg!.className.baseVal).toContain("text-ide-faint");
  });

  it("renders text icon with faint class for 'text/plain'", () => {
    const { container } = render(<ContentIcon contentType="text/plain" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-faint");
  });

  it("renders url icon with info class (PARITY-SPEC §6)", () => {
    const { container } = render(<ContentIcon contentType="url" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // crh3.42: URL uses ide-info token per PARITY-SPEC §6 (matches Android c.info).
    // Reverts 1hqt (sky) which deviated from spec.
    expect(svg!.className.baseVal).toContain("text-ide-info");
    expect(svg!.className.baseVal).not.toContain("text-ide-sky");
  });

  it("renders image icon with violet class for 'image'", () => {
    const { container } = render(<ContentIcon contentType="image" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // 1jms.14: IMAGE → violet per PARITY-SPEC §6 (distinct from URL=sky; matches Android c.violet)
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });

  it("renders image icon with violet class for 'image/png' (MIME prefix)", () => {
    const { container } = render(<ContentIcon contentType="image/png" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });

  it("renders code icon with violet class for 'code'", () => {
    const { container } = render(<ContentIcon contentType="code" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });

  it("renders code icon for 'text/x-python' MIME", () => {
    const { container } = render(<ContentIcon contentType="text/x-python" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });

  it("respects size prop (default 14)", () => {
    const { container } = render(<ContentIcon contentType="text" />);
    const svg = container.querySelector("svg");
    expect(svg!.getAttribute("width")).toBe("14");
    expect(svg!.getAttribute("height")).toBe("14");
  });

  it("respects explicit size prop", () => {
    const { container } = render(<ContentIcon contentType="text" size={20} />);
    const svg = container.querySelector("svg");
    expect(svg!.getAttribute("width")).toBe("20");
    expect(svg!.getAttribute("height")).toBe("20");
  });

  it("renders faint fallback icon for unknown type", () => {
    const { container } = render(<ContentIcon contentType="application/pdf" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // Falls through to the FileText/faint path — violet for application/*
    // (matches the Popup.tsx ContentChip code/application/* → violet branch)
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });
});

// ---------------------------------------------------------------------------
// KindChip
// ---------------------------------------------------------------------------

describe("KindChip", () => {
  it("renders TEXT label with faint class (ICON-2: spec .b-text wants faint/grey, not accent/blue)", () => {
    const { getByText } = render(<KindChip contentType="text" />);
    const el = getByText("TEXT");
    // ICON-2: TEXT badge uses faint/grey — plain text should not look accent-highlighted.
    expect(el.className).toContain("text-ide-faint");
    expect(el.className).not.toContain("text-ide-accent");
  });

  it("renders URL label with info class (PARITY-SPEC §6)", () => {
    const { getByText } = render(<KindChip contentType="url" />);
    const el = getByText("URL");
    // crh3.42: URL chip uses ide-info per PARITY-SPEC §6 (Android c.info).
    expect(el.className).toContain("text-ide-info");
    expect(el.className).not.toContain("text-ide-sky");
  });

  it("renders IMAGE label with violet class", () => {
    const { getByText } = render(<KindChip contentType="image" />);
    const el = getByText("IMAGE");
    expect(el.className).toContain("text-ide-violet");
  });

  it("renders CODE label with violet class", () => {
    const { getByText } = render(<KindChip contentType="code" />);
    const el = getByText("CODE");
    expect(el.className).toContain("text-ide-violet");
  });

  it("prefers explicit kind over contentType-derived label", () => {
    const { getByText } = render(
      <KindChip contentType="text" kind="EMAIL" />
    );
    const el = getByText("EMAIL");
    expect(el.className).toContain("text-ide-success");
  });

  it("renders IMAGE via kind prop", () => {
    const { getByText } = render(
      <KindChip contentType="text" kind="IMAGE" />
    );
    const el = getByText("IMAGE");
    expect(el.className).toContain("text-ide-violet");
  });
});

// ---------------------------------------------------------------------------
// kindFallback — exported helper (CopyPaste-bdac.29)
// ---------------------------------------------------------------------------

describe("kindFallback", () => {
  it("returns 'URL' for 'url'", () => {
    expect(kindFallback("url")).toBe("URL");
  });

  it("returns 'IMAGE' for 'image'", () => {
    expect(kindFallback("image")).toBe("IMAGE");
  });

  it("returns 'IMAGE' for 'image/png' (MIME prefix)", () => {
    expect(kindFallback("image/png")).toBe("IMAGE");
  });

  it("returns 'CODE' for 'code'", () => {
    expect(kindFallback("code")).toBe("CODE");
  });

  it("returns 'CODE' for 'text/x-python'", () => {
    expect(kindFallback("text/x-python")).toBe("CODE");
  });

  it("returns 'CODE' for 'application/json'", () => {
    expect(kindFallback("application/json")).toBe("CODE");
  });

  it("returns 'TEXT' for 'text' (plain text fallback)", () => {
    expect(kindFallback("text")).toBe("TEXT");
  });

  it("returns 'TEXT' for 'file' (no matching branch)", () => {
    // 'file' does not match url/image/code branches — falls through to TEXT
    expect(kindFallback("file")).toBe("TEXT");
  });

  it("returns 'TEXT' for unknown/json string", () => {
    expect(kindFallback("json")).toBe("TEXT");
  });
});

// ---------------------------------------------------------------------------
// density pref in the Zustand store
// ---------------------------------------------------------------------------

describe("store: density pref", () => {
  beforeEach(() => {
    // Reset the store to defaults before each test.
    act(() => {
      useUI.getState().setPrefs({ density: "comfortable" });
    });
  });

  it("defaults to 'comfortable'", () => {
    expect(useUI.getState().prefs.density).toBe("comfortable");
  });

  it("can be set to 'compact' via setPrefs", () => {
    act(() => {
      useUI.getState().setPrefs({ density: "compact" });
    });
    expect(useUI.getState().prefs.density).toBe("compact");
  });

  it("persists round-trip through setPrefs back to 'comfortable'", () => {
    act(() => {
      useUI.getState().setPrefs({ density: "compact" });
    });
    expect(useUI.getState().prefs.density).toBe("compact");
    act(() => {
      useUI.getState().setPrefs({ density: "comfortable" });
    });
    expect(useUI.getState().prefs.density).toBe("comfortable");
  });
});
